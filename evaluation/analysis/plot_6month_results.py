import pandas as pd
import matplotlib.pyplot as plt
import numpy as np
from pathlib import Path

def plot_regime_timeline():
    """Figure 7: Regime state transitions over 6 months."""
    df = pd.read_csv("evaluation/results/regime_timeline.csv")
    
    fig, ax = plt.subplots(figsize=(12, 3))
    
    regime_colors = {0: '#2E7D32', 1: '#F57C00', 2: '#C62828'}  # Calm, PreStorm, Storm
    regime_names = {0: 'Calm', 1: 'PreStorm', 2: 'Storm'}
    
    for regime_code, color in regime_colors.items():
        mask = df['regime'] == regime_code
        ax.scatter(df[mask]['day'], df[mask]['regime'], 
                   c=color, label=regime_names[regime_code], s=10, alpha=0.6)
    
    ax.set_xlabel('Day', fontsize=12)
    ax.set_ylabel('Regime State', fontsize=12)
    ax.set_yticks([0, 1, 2])
    ax.set_yticklabels(['Calm', 'PreStorm', 'Storm'])
    ax.legend(loc='upper right')
    ax.grid(axis='x', alpha=0.3)
    
    plt.tight_layout()
    plt.savefig('evaluation/figures/regime_timeline.pdf', dpi=300)
    plt.savefig('evaluation/figures/regime_timeline.png', dpi=150)
    print("✓ Saved regime_timeline.pdf")

def plot_energy_autonomy():
    """Figure 8: Battery level over 6 months with solar recharge."""
    df = pd.read_csv("evaluation/results/energy_timeline.csv")
    
    fig, ax = plt.subplots(figsize=(10, 4))
    
    ax.plot(df['day'], df['energy_joules'], linewidth=1.5, color='#1976D2')
    ax.axhline(23760, color='green', linestyle='--', label='Full Capacity', alpha=0.5)
    ax.axhline(2376, color='red', linestyle='--', label='Critical (10%)', alpha=0.5)
    
    ax.set_xlabel('Day', fontsize=12)
    ax.set_ylabel('Energy Remaining (J)', fontsize=12)
    ax.set_title('6-Month Battery Autonomy with Solar Recharge', fontsize=14)
    ax.legend()
    ax.grid(alpha=0.3)
    
    # Annotate minimum energy point
    min_idx = df['energy_joules'].idxmin()
    min_day = df.loc[min_idx, 'day']
    min_energy = df.loc[min_idx, 'energy_joules']
    ax.annotate(f'Min: {min_energy:.0f}J\n(Day {min_day:.0f})',
                xy=(min_day, min_energy), xytext=(min_day + 20, min_energy - 3000),
                arrowprops=dict(arrowstyle='->', color='red'), fontsize=10)
    
    plt.tight_layout()
    plt.savefig('evaluation/figures/energy_autonomy.pdf', dpi=300)
    plt.savefig('evaluation/figures/energy_autonomy.png', dpi=150)
    print("✓ Saved energy_autonomy.pdf")

def plot_sleep_adaptation():
    """Figure 9: Dynamic sleep interval adaptation."""
    df = pd.read_csv("evaluation/results/sleep_intervals.csv")
    
    fig, ax = plt.subplots(figsize=(10, 4))
    
    ax.plot(df['day'], df['interval_seconds'] / 3600, linewidth=1.5, color='#7B1FA2')
    ax.set_xlabel('Day', fontsize=12)
    ax.set_ylabel('Sleep Interval (hours)', fontsize=12)
    ax.set_title('TWT Wake Interval Adaptation Over 6 Months', fontsize=14)
    ax.set_yscale('log')
    ax.grid(alpha=0.3, which='both')
    
    # Add regime transition annotations
    regime_df = pd.read_csv("evaluation/results/regime_timeline.csv")
    storm_days = regime_df[regime_df['regime'] == 2]['day'].values
    for day in storm_days[:5]:  # Annotate first 5 storms
        ax.axvline(day, color='red', alpha=0.2, linestyle='--')
    
    plt.tight_layout()
    plt.savefig('evaluation/figures/sleep_adaptation.pdf', dpi=300)
    plt.savefig('evaluation/figures/sleep_adaptation.png', dpi=150)
    print("✓ Saved sleep_adaptation.pdf")

def generate_latex_summary():
    """Generate LaTeX table of long-term metrics for paper."""
    regime_df = pd.read_csv("evaluation/results/regime_timeline.csv")
    energy_df = pd.read_csv("evaluation/results/energy_timeline.csv")
    
    total_days = regime_df['day'].max()
    calm_pct = 100 * (regime_df['regime'] == 0).sum() / len(regime_df)
    prestorm_pct = 100 * (regime_df['regime'] == 1).sum() / len(regime_df)
    storm_pct = 100 * (regime_df['regime'] == 2).sum() / len(regime_df)
    
    min_energy = energy_df['energy_joules'].min()
    final_energy = energy_df['energy_joules'].iloc[-1]
    
    latex = r"""
\begin{table}[h]
\centering
\begin{tabular}{lr}
\toprule
Metric & Value \\
\midrule
Simulated Duration & %.0f days \\
Calm Regime & %.1f\%% \\
PreStorm Regime & %.1f\%% \\
Storm Regime & %.1f\%% \\
Minimum Energy & %.0f J (%.1f\%%) \\
Final Energy & %.0f J (%.1f\%%) \\
\bottomrule
\end{tabular}
\caption{6-Month Long-Term Autonomy Metrics}
\label{tab:longterm}
\end{table}
""" % (total_days, calm_pct, prestorm_pct, storm_pct,
       min_energy, 100 * min_energy / 23760,
       final_energy, 100 * final_energy / 23760)
    
    with open("evaluation/results/longterm_summary.tex", "w") as f:
        f.write(latex)
    
    print("\n✓ LaTeX table saved to evaluation/results/longterm_summary.tex")
    print(f"\nKey findings:")
    print(f"  - Survived {total_days:.0f} days with {final_energy:.0f}J remaining")
    print(f"  - Spent {calm_pct:.1f}% in Calm (aggressive power saving)")
    print(f"  - Minimum energy: {min_energy:.0f}J ({100*min_energy/23760:.1f}% capacity)")

if __name__ == "__main__":
    Path("evaluation/figures").mkdir(parents=True, exist_ok=True)
    
    plot_regime_timeline()
    plot_energy_autonomy()
    plot_sleep_adaptation()
    generate_latex_summary()
    
    print("\n=== All figures generated ===")
    print("Add these to your paper as:")
    print("  Figure 7: Regime Timeline")
    print("  Figure 8: Energy Autonomy")
    print("  Figure 9: Sleep Interval Adaptation")
