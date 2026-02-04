"""
Generate Class C detection timeline figure for QRES v20.0.1 paper
Based on verified simulation results from class_c_collusion_sim.py
"""

import matplotlib.pyplot as plt
import numpy as np

# Verified detection data from simulation runs
detection_rounds = [31, 32, 36, 43, 49, 57, 113, 123, 165, 174]
cartel_ids = [1, 9, 7, 4, 8, 5, 3, 6, 2, 0]

# Create figure
fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(10, 8), sharex=True)

# Top plot: Cumulative detection
cumulative_detected = np.arange(1, 11)
ax1.plot(detection_rounds, cumulative_detected, 'ro-', linewidth=2, markersize=8, label='Cartel Members Detected')
ax1.axhline(y=10, color='gray', linestyle='--', alpha=0.5, label='Total Cartel Size')
ax1.fill_between(detection_rounds, 0, cumulative_detected, alpha=0.2, color='red')
ax1.set_ylabel('Cumulative Detections', fontsize=12)
ax1.set_title('QRES v20.0.1: Class C Collusion Detection Timeline\n(100% detection, 0% false positives, 2% bandwidth overhead)', fontsize=13, fontweight='bold')
ax1.grid(True, alpha=0.3)
ax1.legend(loc='lower right', fontsize=10)
ax1.set_ylim(0, 11)

# Add milestones
ax1.axvline(x=50, color='blue', linestyle=':', alpha=0.6, linewidth=1.5)
ax1.text(50, 9.5, '50% detected\n(Round 50)', fontsize=9, ha='left', va='top', color='blue')
ax1.axvline(x=100, color='green', linestyle=':', alpha=0.6, linewidth=1.5)
ax1.text(100, 8, '60% detected\n(Round 100)', fontsize=9, ha='left', va='top', color='green')

# Bottom plot: Detection events
for i, (round_num, cartel_id) in enumerate(zip(detection_rounds, cartel_ids)):
    ax2.scatter(round_num, cartel_id, s=150, c='red', marker='X', zorder=3, edgecolor='darkred', linewidth=1.5)
    if i == 0:
        ax2.scatter(round_num, cartel_id, s=150, c='red', marker='X', zorder=3, label='Detection Event', edgecolor='darkred', linewidth=1.5)

ax2.set_xlabel('Round Number', fontsize=12)
ax2.set_ylabel('Cartel Node ID', fontsize=12)
ax2.set_yticks(range(10))
ax2.set_xlim(0, 200)
ax2.grid(True, alpha=0.3, axis='x')
ax2.legend(loc='upper right', fontsize=10)

# Add statistics box
stats_text = (
    f"Detection Statistics:\n"
    f"  • First: Round {min(detection_rounds)}\n"
    f"  • Last: Round {max(detection_rounds)}\n"
    f"  • Mean: Round {np.mean(detection_rounds):.1f}\n"
    f"  • Audit Rate: 2.0%\n"
    f"  • False Positives: 0/390"
)
ax2.text(0.98, 0.02, stats_text, transform=ax2.transAxes,
         fontsize=9, verticalalignment='bottom', horizontalalignment='right',
         bbox=dict(boxstyle='round', facecolor='wheat', alpha=0.8))

plt.tight_layout()

# Save to paper figures directory
output_path = '../docs/RaaS_Paper/figures/class_c_detection_timeline.png'
plt.savefig(output_path, dpi=300, bbox_inches='tight')
print(f"[SUCCESS] Figure saved to {output_path}")

# Also save as PDF for LaTeX
output_path_pdf = '../docs/RaaS_Paper/figures/class_c_detection_timeline.pdf'
plt.savefig(output_path_pdf, bbox_inches='tight')
print(f"[SUCCESS] PDF version saved to {output_path_pdf}")

plt.show()
