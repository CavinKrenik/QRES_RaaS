import pandas as pd
import numpy as np
from pathlib import Path

def extract_storm_indices():
    # Load regime timeline
    regime_df = pd.read_csv("docs/RaaS_Data/regime_timeline.csv")
    storm_days = regime_df[regime_df['regime'] == 2]['day'].values
    
    if len(storm_days) == 0:
        print("No Storm regimes found!")
        return None
    
    # Load weather data
    # Find the csv file
    data_dir = Path("evaluation/data/long_term")
    csv_files = list(data_dir.glob("weather_6month_*.csv"))
    if not csv_files:
        print("Weather file not found!")
        return None
    weather_df = pd.read_csv(csv_files[0])
    
    # Map days to indices (144 samples per day)
    storm_indices = []
    for day in storm_days:
        start_idx = int(day * 144)
        end_idx = start_idx + 144
        storm_indices.extend(range(start_idx, min(end_idx, len(weather_df))))
    
    # Extract features for these indices
    storm_data = weather_df.iloc[storm_indices].copy()
    
    # Normalize features as per QRES logic
    # T (degC), p (mbar), wv (m/s)
    storm_data['T_norm'] = (storm_data['T (degC)'] + 20.0) / 40.0
    storm_data['P_norm'] = (storm_data['p (mbar)'] - 950.0) / 100.0
    storm_data['W_norm'] = storm_data['wv (m/s)'] / 30.0
    
    features = storm_data[['T_norm', 'P_norm', 'W_norm']].values
    
    # Calculate global std dev of these features during storm
    std_dev = np.std(features)
    print(f"Storm Std Dev (Global): {std_dev:.4f}")
    
    # Also calculate mean std dev of windows (simulation uses window=10)
    window_stds = []
    for i in range(0, len(features)-10, 10):
        window = features[i:i+10]
        window_stds.append(np.std(window))
    
    mean_window_std = np.mean(window_stds) if window_stds else std_dev
    print(f"Storm Mean Window Std Dev: {mean_window_std:.4f}")
    
    return mean_window_std

if __name__ == "__main__":
    extract_storm_indices()
