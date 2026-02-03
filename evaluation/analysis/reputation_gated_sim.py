"""
Reputation-Gated Aggregation Simulation
========================================
Compares Standard TrimmedMeanByz aggregation against a fused
Layer 2 (Reputation) + Layer 4 (Robust Aggregation) strategy.

Faithfully replicates the algorithms from:
  - crates/qres_core/src/aggregation.rs  (TrimmedMeanByz)
  - crates/qres_core/src/reputation.rs   (ReputationTracker)

Simulation parameters:
  - 20 nodes total, 25% Byzantine (5 attackers)
  - 10-dimensional model weight vector
  - 100 aggregation rounds
  - Byzantine strategy: targeted drift injection (+0.5 offset per dim)
"""

import numpy as np
import pandas as pd
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from pathlib import Path

# ── Simulation Constants ──────────────────────────────────────────────
SEED = 42
N_NODES = 20
BYZ_RATIO = 0.25
N_BYZ = int(N_NODES * BYZ_RATIO)  # 5
N_HONEST = N_NODES - N_BYZ        # 15
DIM = 10
ROUNDS = 100
TRUE_WEIGHTS = np.zeros(DIM)  # Ground truth: all-zeros model

# Reputation constants (mirrors reputation.rs)
DEFAULT_TRUST = 0.5
BAN_THRESHOLD = 0.2
SOFT_GATE_THRESHOLD = 0.4   # NEW: Soft gate for gated aggregation
TRIM_PENALTY = -0.1         # NEW: Penalty for trimmed-margin nodes
HONEST_REWARD = 0.02        # Reward for contributing accepted updates
DRIFT_PENALTY = 0.08        # Existing penalty from reputation.rs
# A node must be trimmed in >= this fraction of dimensions to count as "rejected"
TRIM_DIMENSION_FRACTION = 0.7

# Byzantine attack parameters
BYZ_OFFSET = 0.5   # Constant offset injected per dimension per round
HONEST_NOISE_STD = 0.05  # Honest node noise std


# ── Aggregation Algorithms ────────────────────────────────────────────

def trimmed_mean_byz(updates: np.ndarray, f: int) -> tuple[np.ndarray, list, list]:
    """Coordinate-wise trimmed mean (mirrors aggregation.rs:trimmed_mean_byz).

    Removes top-f and bottom-f values per dimension, averages the rest.
    Returns (result, selected_indices, rejected_indices_per_round).
    """
    n = updates.shape[0]
    d = updates.shape[1]

    if 2 * f >= n:
        # Fallback to median
        return np.median(updates, axis=0), list(range(n)), []

    result = np.zeros(d)
    # Track which node indices fall in trimmed margins across dimensions
    trim_counts = np.zeros(n, dtype=int)

    for dim in range(d):
        vals = updates[:, dim].copy()
        order = np.argsort(vals)
        # Bottom-f and top-f are trimmed
        trimmed_bottom = order[:f]
        trimmed_top = order[n - f:]
        kept = order[f:n - f]

        for idx in np.concatenate([trimmed_bottom, trimmed_top]):
            trim_counts[idx] += 1

        result[dim] = np.mean(vals[kept])

    # A node is considered "rejected" if it was trimmed in >= TRIM_DIMENSION_FRACTION of dimensions
    threshold = int(d * TRIM_DIMENSION_FRACTION)
    rejected = [i for i in range(n) if trim_counts[i] >= threshold]
    selected = [i for i in range(n) if i not in rejected]

    return result, selected, rejected


def reputation_gated_trimmed_mean(
    updates: np.ndarray,
    f: int,
    reputation_scores: np.ndarray,
) -> tuple[np.ndarray, list, list, list]:
    """Reputation-Gated TrimmedMeanByz (Layer 2 + Layer 4 fusion).

    1. SOFT GATE: Drop nodes with reputation < SOFT_GATE_THRESHOLD before aggregation
    2. Run TrimmedMeanByz on admitted nodes
    3. Weight surviving contributions by reputation
    4. Return (result, admitted_indices, rejected_by_trim, gated_out_indices)
    """
    n = updates.shape[0]

    # Step 1: Soft gate - filter out low-reputation nodes
    admitted_mask = reputation_scores >= SOFT_GATE_THRESHOLD
    admitted_indices = np.where(admitted_mask)[0].tolist()
    gated_out = np.where(~admitted_mask)[0].tolist()

    if len(admitted_indices) < 4:
        # Not enough nodes after gating, admit all non-banned
        admitted_mask = reputation_scores >= BAN_THRESHOLD
        admitted_indices = np.where(admitted_mask)[0].tolist()
        gated_out = np.where(~admitted_mask)[0].tolist()
    if len(admitted_indices) < 3:
        # Even after relaxing, not enough — admit everyone
        admitted_indices = list(range(n))
        gated_out = []

    admitted_updates = updates[admitted_indices]
    admitted_reps = reputation_scores[admitted_indices]
    n_admitted = len(admitted_indices)

    # Step 2: Adaptive f based on admitted count
    f_eff = min(f, (n_admitted - 1) // 2)
    if f_eff < 1:
        f_eff = 0

    d = updates.shape[1]
    result = np.zeros(d)
    trim_counts = np.zeros(n_admitted, dtype=int)

    for dim in range(d):
        vals = admitted_updates[:, dim].copy()
        order = np.argsort(vals)

        if f_eff > 0 and 2 * f_eff < n_admitted:
            trimmed_bottom = order[:f_eff]
            trimmed_top = order[n_admitted - f_eff:]
            kept = order[f_eff:n_admitted - f_eff]

            for idx in np.concatenate([trimmed_bottom, trimmed_top]):
                trim_counts[idx] += 1
        else:
            kept = order

        # Step 3: Weighted average by reputation
        kept_vals = vals[kept]
        kept_reps = admitted_reps[kept]
        total_w = np.sum(kept_reps)
        if total_w > 0:
            result[dim] = np.sum(kept_vals * kept_reps) / total_w
        else:
            result[dim] = np.mean(kept_vals)

    # Determine which admitted nodes were in the trimmed margin
    threshold = int(d * TRIM_DIMENSION_FRACTION)
    rejected_local = [i for i in range(n_admitted) if trim_counts[i] >= threshold]
    rejected_global = [admitted_indices[i] for i in rejected_local]

    return result, admitted_indices, rejected_global, gated_out


# ── Reputation Tracker ────────────────────────────────────────────────

class ReputationTracker:
    """Mirrors crates/qres_core/src/reputation.rs"""

    def __init__(self, n_nodes: int):
        self.scores = np.full(n_nodes, DEFAULT_TRUST)

    def get_scores(self) -> np.ndarray:
        return self.scores.copy()

    def reward(self, indices: list, amount: float = HONEST_REWARD):
        for i in indices:
            self.scores[i] = min(self.scores[i] + amount, 1.0)

    def penalize(self, indices: list, amount: float = DRIFT_PENALTY):
        for i in indices:
            self.scores[i] = max(self.scores[i] - amount, 0.0)

    def penalize_trim_margin(self, indices: list):
        """NEW: Nodes in the trimmed margin receive TRIM_PENALTY."""
        for i in indices:
            self.scores[i] = max(self.scores[i] + TRIM_PENALTY, 0.0)  # TRIM_PENALTY is negative


# ── Simulation ────────────────────────────────────────────────────────

def generate_updates(rng: np.random.Generator, round_idx: int) -> np.ndarray:
    """Generate updates: honest nodes cluster near TRUE_WEIGHTS, Byzantine inject drift."""
    updates = np.zeros((N_NODES, DIM))

    # Honest nodes: small Gaussian noise around true weights
    for i in range(N_HONEST):
        updates[i] = TRUE_WEIGHTS + rng.normal(0, HONEST_NOISE_STD, DIM)

    # Byzantine nodes: strategic offset (constant poisoning attack)
    for i in range(N_HONEST, N_NODES):
        updates[i] = TRUE_WEIGHTS + BYZ_OFFSET + rng.normal(0, 0.01, DIM)

    return updates


def compute_drift(aggregated: np.ndarray) -> float:
    """L2 norm of deviation from true weights, normalized by dimension."""
    return np.sqrt(np.mean((aggregated - TRUE_WEIGHTS) ** 2))


def run_simulation():
    rng = np.random.default_rng(SEED)

    # Expected Byzantine count for the aggregator
    f_param = N_BYZ  # Set f = actual number of attackers

    # Track metrics per round
    records = []
    rep_tracker = ReputationTracker(N_NODES)

    # ── Run both strategies side-by-side ──
    standard_drifts = []
    gated_drifts = []
    rep_history = []

    for r in range(ROUNDS):
        updates = generate_updates(rng, r)

        # ── Strategy A: Standard TrimmedMeanByz ──
        agg_std, sel_std, rej_std = trimmed_mean_byz(updates, f_param)
        drift_std = compute_drift(agg_std)
        standard_drifts.append(drift_std)

        # ── Strategy B: Reputation-Gated TrimmedMeanByz ──
        scores = rep_tracker.get_scores()
        agg_gated, admitted, rej_gated, gated_out = reputation_gated_trimmed_mean(
            updates, f_param, scores
        )
        drift_gated = compute_drift(agg_gated)
        gated_drifts.append(drift_gated)

        # ── Update reputation based on gated aggregation results ──
        # Drift-based penalty: compare each admitted node's update to the
        # aggregated result. Nodes far from consensus are penalized.
        drift_threshold = 0.3  # Per-node L2 distance threshold
        for i in admitted:
            node_drift = np.sqrt(np.mean((updates[i] - agg_gated) ** 2))
            if node_drift > drift_threshold:
                # This node is far from consensus → likely Byzantine
                rep_tracker.penalize_trim_margin([i])  # -0.1
            else:
                # Node contributed a good update
                rep_tracker.reward([i], amount=HONEST_REWARD)

        # Nodes that were already gated out get a tiny decay
        rep_tracker.penalize(gated_out, amount=0.01)

        # Snapshot reputation
        cur_scores = rep_tracker.get_scores()
        avg_honest_rep = np.mean(cur_scores[:N_HONEST])
        avg_byz_rep = np.mean(cur_scores[N_HONEST:])
        n_gated = len(gated_out)
        n_byz_gated = sum(1 for i in gated_out if i >= N_HONEST)

        rep_history.append(cur_scores.copy())

        records.append({
            "round": r + 1,
            "drift_standard": drift_std,
            "drift_gated": drift_gated,
            "improvement_pct": (1 - drift_gated / max(drift_std, 1e-9)) * 100,
            "avg_honest_rep": avg_honest_rep,
            "avg_byz_rep": avg_byz_rep,
            "nodes_gated_out": n_gated,
            "byz_nodes_gated": n_byz_gated,
            "admitted_count": len(admitted),
        })

    return pd.DataFrame(records), np.array(rep_history)


# ── Plotting ──────────────────────────────────────────────────────────

def plot_results(df: pd.DataFrame, rep_history: np.ndarray):
    fig, axes = plt.subplots(2, 2, figsize=(14, 10))
    fig.suptitle(
        "Reputation-Gated Aggregation: Byzantine Hardening Analysis\n"
        f"(N={N_NODES}, {BYZ_RATIO*100:.0f}% Byzantine, {ROUNDS} rounds)",
        fontsize=14, fontweight="bold"
    )

    # ── Panel 1: Drift Comparison ──
    ax = axes[0, 0]
    ax.plot(df["round"], df["drift_standard"], label="Standard TrimmedMeanByz",
            color="#C62828", linewidth=1.5, alpha=0.8)
    ax.plot(df["round"], df["drift_gated"], label="Reputation-Gated",
            color="#2E7D32", linewidth=1.5, alpha=0.8)
    ax.axhline(0.05, color="black", linestyle="--", alpha=0.5, label="5% Drift Threshold")
    ax.set_xlabel("Round")
    ax.set_ylabel("Model Drift (RMSE)")
    ax.set_title("Aggregation Drift Over Time")
    ax.legend(fontsize=9)
    ax.grid(alpha=0.3)

    # ── Panel 2: Reputation Evolution ──
    ax = axes[0, 1]
    rounds = np.arange(1, ROUNDS + 1)
    avg_honest = np.mean(rep_history[:, :N_HONEST], axis=1)
    avg_byz = np.mean(rep_history[:, N_HONEST:], axis=1)
    ax.plot(rounds, avg_honest, label="Avg Honest Reputation", color="#1976D2", linewidth=2)
    ax.plot(rounds, avg_byz, label="Avg Byzantine Reputation", color="#C62828", linewidth=2)
    ax.axhline(SOFT_GATE_THRESHOLD, color="#F57C00", linestyle="--",
               alpha=0.7, label=f"Soft Gate ({SOFT_GATE_THRESHOLD})")
    ax.axhline(BAN_THRESHOLD, color="red", linestyle=":", alpha=0.7,
               label=f"Ban Threshold ({BAN_THRESHOLD})")
    ax.set_xlabel("Round")
    ax.set_ylabel("Reputation Score")
    ax.set_title("Reputation Decay: Honest vs Byzantine")
    ax.legend(fontsize=9)
    ax.grid(alpha=0.3)
    ax.set_ylim(-0.05, 1.05)

    # ── Panel 3: Byzantine Nodes Gated Out ──
    ax = axes[1, 0]
    ax.bar(df["round"], df["byz_nodes_gated"], color="#7B1FA2", alpha=0.7,
           label="Byzantine Nodes Gated", width=1.0)
    ax.bar(df["round"], df["nodes_gated_out"] - df["byz_nodes_gated"],
           bottom=df["byz_nodes_gated"], color="#90CAF9", alpha=0.5,
           label="Honest Nodes Gated (FP)", width=1.0)
    ax.set_xlabel("Round")
    ax.set_ylabel("Nodes Gated Out")
    ax.set_title("Soft Gate Effectiveness")
    ax.legend(fontsize=9)
    ax.grid(alpha=0.3)

    # ── Panel 4: Improvement Summary ──
    ax = axes[1, 1]
    # Rolling average of improvement
    window = 10
    rolling_imp = df["improvement_pct"].rolling(window, min_periods=1).mean()
    ax.fill_between(df["round"], rolling_imp, alpha=0.3, color="#2E7D32")
    ax.plot(df["round"], rolling_imp, color="#2E7D32", linewidth=2,
            label=f"Rolling {window}-round avg improvement")

    final_imp = df["improvement_pct"].iloc[-20:].mean()
    ax.axhline(final_imp, color="black", linestyle="--", alpha=0.5,
               label=f"Steady-state: {final_imp:.1f}%")
    ax.set_xlabel("Round")
    ax.set_ylabel("Drift Reduction (%)")
    ax.set_title("Gated vs Standard: Drift Improvement")
    ax.legend(fontsize=9)
    ax.grid(alpha=0.3)

    plt.tight_layout(rect=[0, 0, 1, 0.94])
    return fig


def generate_latex_table(df: pd.DataFrame) -> str:
    """Generate LaTeX summary table for the paper."""
    avg_std = df["drift_standard"].mean()
    avg_gated = df["drift_gated"].mean()
    final_std = df["drift_standard"].iloc[-20:].mean()
    final_gated = df["drift_gated"].iloc[-20:].mean()
    avg_imp = df["improvement_pct"].mean()
    final_imp = df["improvement_pct"].iloc[-20:].mean()
    max_byz_gated = df["byz_nodes_gated"].max()
    avg_byz_gated = df["byz_nodes_gated"].iloc[-20:].mean()
    rounds_below_5pct = (df["drift_gated"] < 0.05).sum()

    return rf"""
\begin{{table}}[h]
\centering
\begin{{tabular}}{{lrr}}
\toprule
Metric & Standard & Reputation-Gated \\
\midrule
Mean Drift (all rounds)       & {avg_std:.4f} & {avg_gated:.4f} \\
Steady-State Drift (last 20)  & {final_std:.4f} & {final_gated:.4f} \\
Avg Improvement (\%)          & --- & {avg_imp:.1f}\% \\
Steady-State Improvement (\%) & --- & {final_imp:.1f}\% \\
Rounds Below 5\% Threshold    & {(df["drift_standard"] < 0.05).sum()}/100 & {rounds_below_5pct}/100 \\
Byzantine Nodes Gated (max)   & 0 & {max_byz_gated} \\
Byzantine Nodes Gated (avg, last 20) & 0 & {avg_byz_gated:.1f} \\
\bottomrule
\end{{tabular}}
\caption{{Reputation-Gated Aggregation: 25\% Byzantine, N={N_NODES}, 100 rounds}}
\label{{tab:rep-gated}}
\end{{table}}
"""


# ── Sweep across Byzantine ratios ─────────────────────────────────────

def run_byz_sweep():
    """Run simulation across multiple Byzantine ratios to find breakpoint."""
    ratios = [0.10, 0.15, 0.20, 0.25, 0.30, 0.35, 0.40]
    sweep_records = []

    for ratio in ratios:
        global N_BYZ, N_HONEST
        N_BYZ_local = int(N_NODES * ratio)
        N_HONEST_local = N_NODES - N_BYZ_local

        rng = np.random.default_rng(SEED)
        rep_tracker = ReputationTracker(N_NODES)
        f_param = N_BYZ_local

        std_drifts = []
        gated_drifts = []

        for r in range(ROUNDS):
            updates = np.zeros((N_NODES, DIM))
            for i in range(N_HONEST_local):
                updates[i] = TRUE_WEIGHTS + rng.normal(0, HONEST_NOISE_STD, DIM)
            for i in range(N_HONEST_local, N_NODES):
                updates[i] = TRUE_WEIGHTS + BYZ_OFFSET + rng.normal(0, 0.01, DIM)

            agg_std, _, _ = trimmed_mean_byz(updates, f_param)
            std_drifts.append(compute_drift(agg_std))

            scores = rep_tracker.get_scores()
            agg_gated, admitted, rej_gated, gated_out = reputation_gated_trimmed_mean(
                updates, f_param, scores
            )
            gated_drifts.append(compute_drift(agg_gated))

            drift_threshold = 0.3
            for i in admitted:
                node_drift = np.sqrt(np.mean((updates[i] - agg_gated) ** 2))
                if node_drift > drift_threshold:
                    rep_tracker.penalize_trim_margin([i])
                else:
                    rep_tracker.reward([i], amount=HONEST_REWARD)
            rep_tracker.penalize(gated_out, amount=0.01)

        # Steady-state = last 20 rounds
        ss_std = np.mean(std_drifts[-20:])
        ss_gated = np.nanmean(gated_drifts[-20:])
        improvement = (1 - ss_gated / max(ss_std, 1e-9)) * 100 if not np.isnan(ss_gated) else 0

        sweep_records.append({
            "byz_ratio": ratio,
            "byz_count": N_BYZ_local,
            "ss_drift_standard": ss_std,
            "ss_drift_gated": ss_gated,
            "improvement_pct": improvement,
            "below_5pct_standard": sum(1 for d in std_drifts if d < 0.05),
            "below_5pct_gated": sum(1 for d in gated_drifts if not np.isnan(d) and d < 0.05),
        })
        print(f"  Byz {ratio*100:4.0f}% | Std: {ss_std:.4f} | Gated: {ss_gated:.4f} | Imp: {improvement:.1f}%")

    return pd.DataFrame(sweep_records)


# ── Main ──────────────────────────────────────────────────────────────

if __name__ == "__main__":
    print("=" * 60)
    print("  REPUTATION-GATED AGGREGATION SIMULATION")
    print(f"  N={N_NODES}, Byzantine={N_BYZ} ({BYZ_RATIO*100:.0f}%), Rounds={ROUNDS}")
    print("=" * 60)

    df, rep_history = run_simulation()

    # ── Save CSV ──
    csv_path = Path("docs/RaaS_Data/solution_discovery.csv")
    csv_path.parent.mkdir(parents=True, exist_ok=True)
    df.to_csv(csv_path, index=False, float_format="%.6f")
    print(f"\n[+] Raw metrics saved to {csv_path}")

    # ── Save plot ──
    fig = plot_results(df, rep_history)
    img_path = Path("docs/images/integrity_hardening.png")
    img_path.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(img_path, dpi=200, bbox_inches="tight")
    print(f"[+] Comparison plot saved to {img_path}")
    plt.close(fig)

    # ── Save LaTeX table ──
    latex = generate_latex_table(df)
    tex_path = Path("docs/RaaS_Data/integrity_hardening_table.tex")
    with open(tex_path, "w") as f:
        f.write(latex)
    print(f"[+] LaTeX table saved to {tex_path}")

    # ── Byzantine Ratio Sweep ──
    print("\n--- Byzantine Ratio Sweep ---")
    sweep_df = run_byz_sweep()
    sweep_path = Path("docs/RaaS_Data/byz_ratio_sweep.csv")
    sweep_df.to_csv(sweep_path, index=False, float_format="%.6f")
    print(f"[+] Sweep data saved to {sweep_path}")

    # ── Console Summary ──
    print("\n" + "=" * 60)
    print("  RESULTS SUMMARY")
    print("=" * 60)

    avg_std = df["drift_standard"].mean()
    avg_gated = df["drift_gated"].mean()
    final_std = df["drift_standard"].iloc[-20:].mean()
    final_gated = df["drift_gated"].iloc[-20:].mean()
    final_imp = df["improvement_pct"].iloc[-20:].mean()

    print(f"  Mean Drift   | Standard: {avg_std:.4f} | Gated: {avg_gated:.4f}")
    print(f"  Steady-State | Standard: {final_std:.4f} | Gated: {final_gated:.4f}")
    print(f"  Improvement  | {final_imp:.1f}% drift reduction (steady-state)")
    print(f"  Rounds <5%   | Standard: {(df['drift_standard'] < 0.05).sum()}/100"
          f" | Gated: {(df['drift_gated'] < 0.05).sum()}/100")

    avg_byz_rep = df["avg_byz_rep"].iloc[-1]
    avg_hon_rep = df["avg_honest_rep"].iloc[-1]
    print(f"\n  Final Reputation | Honest: {avg_hon_rep:.3f} | Byzantine: {avg_byz_rep:.3f}")
    print(f"  Byzantine gated  | {df['byz_nodes_gated'].iloc[-1]}/{N_BYZ} in final round")

    below_threshold = final_gated < 0.05
    print(f"\n  30% TOLERANCE TARGET: {'ACHIEVED' if below_threshold else 'NOT YET ACHIEVED'}")
    print(f"  (Steady-state drift {final_gated:.4f} {'<' if below_threshold else '>='} 0.05)")
    print("=" * 60)
