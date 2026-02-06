"""
QRES Scalability Visualization

Generates a publication-ready chart showing memory usage and success rate
across node counts from the swarm_scale benchmark.

Usage:
    python plot_scale.py
"""

import pandas as pd
import matplotlib.pyplot as plt

# Load Data
df = pd.read_csv('scalability_massive.csv')

fig, ax1 = plt.subplots(figsize=(10, 6))

# Plot Memory (Bar)
bars = ax1.bar(
    df['nodes'].astype(str), 
    df['memory_mb'], 
    color='#4a90e2', 
    alpha=0.7, 
    label='Total RAM Usage (MB)'
)
ax1.set_ylabel('Total Memory (MB)', color='#4a90e2', fontweight='bold')
ax1.set_xlabel('Simulated Nodes', fontweight='bold')
ax1.tick_params(axis='y', labelcolor='#4a90e2')

# Plot Success Rate (Line)
ax2 = ax1.twinx()
line = ax2.plot(
    df['nodes'].astype(str), 
    df['success_rate'], 
    color='#e74c3c', 
    marker='o', 
    linewidth=3, 
    label='Success Rate (%)'
)
ax2.set_ylabel('Success Rate (%)', color='#e74c3c', fontweight='bold')
ax2.set_ylim(0, 110)
ax2.tick_params(axis='y', labelcolor='#e74c3c')

# Title and styling
plt.title('QRES Scalability: 10,000 Node Simulation (Single vCPU)', fontsize=14, fontweight='bold')
ax1.grid(True, alpha=0.3, axis='y')

# Legend
lines1, labels1 = ax1.get_legend_handles_labels()
lines2, labels2 = ax2.get_legend_handles_labels()
ax1.legend(lines1 + lines2, labels1 + labels2, loc='upper left')

plt.tight_layout()
plt.savefig('scalability_chart.png', dpi=300, bbox_inches='tight')
print("✅ Chart saved to scalability_chart.png")
