import pandas as pd
import numpy as np
from pathlib import Path
import urllib.request
import zipfile

def fetch_jena_6month(output_dir: Path, start_date: str = "2016-01-01", months: int = 6):
    """
    Fetch 6 months of Jena Climate data for long-term QRES testing.
    """
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # Download full Jena dataset
    csv_path = output_dir / "jena_climate_2009_2016.csv"
    if not csv_path.exists():
        print("Downloading Jena Climate dataset...")
        url = "https://storage.googleapis.com/tensorflow/tf-keras-datasets/jena_climate_2009_2016.csv.zip"
        zip_path = output_dir / "jena.zip"
        urllib.request.urlretrieve(url, zip_path)
        
        with zipfile.ZipFile(zip_path) as z:
            z.extractall(output_dir)
    
    # Load and filter
    df = pd.read_csv(csv_path)
    df['Date Time'] = pd.to_datetime(df['Date Time'], format='%d.%m.%Y %H:%M:%S')
    
    start = pd.to_datetime(start_date)
    end = start + pd.DateOffset(months=months)
    
    df_filtered = df[(df['Date Time'] >= start) & (df['Date Time'] < end)].copy()
    
    print(f"Extracted {len(df_filtered)} samples from {start.date()} to {end.date()}")
    
    # Inject aggressive storms
    df_filtered = inject_aggressive_storms(df_filtered)
    
    # Export
    export_path = output_dir / f"weather_6month_{start_date}.csv"
    df_filtered.to_csv(export_path, index=False)
    
    generate_summary_stats(df_filtered, output_dir)
    
    return export_path

def inject_aggressive_storms(df: pd.DataFrame) -> pd.DataFrame:
    """
    Inject intense storm events to force Regime transitions.
    Targeting 10-15% non-Calm duration.
    """
    df = df.reset_index(drop=True)
    
    # Inject 8-12 major storms (approx 2 per month)
    storm_count = np.random.randint(8, 13)
    
    print(f"Injecting {storm_count} aggressive storms...")
    
    valid_range = len(df) - 4000
    
    for i in range(storm_count):
        # Random start, ensure some spacing
        start_idx = np.random.randint(1000, valid_range)
        duration = np.random.randint(144, 720) # 1 to 5 days
        
        # Apply modifiers
        # 10% Pressure drop (Significantly lower pressure triggers 'PreStorm')
        df.loc[start_idx:start_idx+duration, 'p (mbar)'] *= 0.90
        
        # 4x Wind Speed (chaotic, varying)
        noise = np.random.normal(1.0, 0.5, duration + 1)
        wind_mult = 4.0 * np.abs(noise)
        df.loc[start_idx:start_idx+duration, 'wv (m/s)'] *= wind_mult
        
        # 10C Temp drop
        df.loc[start_idx:start_idx+duration, 'T (degC)'] -= 10.0
        
    return df

def generate_summary_stats(df: pd.DataFrame, output_dir: Path):
    """Generate stats."""
    stats = {
        'Total Samples': len(df),
        'Duration (days)': (df['Date Time'].max() - df['Date Time'].min()).days,
        'Temp Range': f"{df['T (degC)'].min():.1f} to {df['T (degC)'].max():.1f}",
        'Pressure Range': f"{df['p (mbar)'].min():.1f} to {df['p (mbar)'].max():.1f}",
        'Max Wind': f"{df['wv (m/s)'].max():.1f}",
    }
    
    with open(output_dir / "dataset_stats.tex", "w") as f:
        for k, v in stats.items():
            f.write(f"{k}: {v}\n")
    
    print("\nStats updated.")

if __name__ == "__main__":
    output = Path("evaluation/data/long_term") 
    fetch_jena_6month(output)
