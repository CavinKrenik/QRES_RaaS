
import numpy as np
import sys
import os
import matplotlib.pyplot as plt

# Configuration
N_TOTAL = 15
F_BYZANTINE = 5  
BIAS_LEVELS = [0.0, 0.05, 0.10, 0.15, 0.20, 0.25, 0.30, 0.35]
N_TRIALS = 30
DRIFT_THRESHOLD = 0.05
SIGMA_MULTIPLIER = 3.0
MAX_ROUNDS = 50
GENE_DIMENSIONS = 8
HONEST_STD = 0.02

def euclidean_dist_sq(v1, v2):
    return np.sum((v1 - v2) ** 2)

def real_multi_krum(vectors, f):
    n = len(vectors)
    k = n - f - 2
    if k < 1: 
        # Fallback to mean if too few
        return np.mean(vectors, axis=0), [], []
        
    # Compute pairwise distances
    # vectors: (n, d)
    # dists[i,j] = ||v_i - v_j||^2
    diff = vectors[:, np.newaxis, :] - vectors[np.newaxis, :, :]
    dists = np.sum(diff ** 2, axis=2)
    
    scores = []
    for i in range(n):
        # Sort distances for node i
        # First is always 0 (dist to self)
        # We sum k nearest neighbors (excluding self)
        # ds[1] is 1st neighbor, ds[k] is kth neighbor
        ds = np.sort(dists[i])
        score = np.sum(ds[1:k+1]) 
        scores.append((score, i))
        
    scores.sort(key=lambda x: x[0])
    
    # Select top m = n - f
    m = n - f
    best_indices = [idx for s, idx in scores[:m]]
    selected = vectors[best_indices]
    
    return np.mean(selected, axis=0), best_indices, []

def generate_honest_nodes(n_honest, center, std_dev, rng):
    return rng.normal(center, std_dev, (n_honest, GENE_DIMENSIONS))

def generate_malicious_nodes(n_malicious, honest_nodes, bias_level, sigma_multiplier, rng):
    if n_malicious == 0: return np.array([]).reshape(0, GENE_DIMENSIONS)
    honest_mean = np.mean(honest_nodes, axis=0)
    honest_std = np.std(honest_nodes, axis=0)
    attack_direction = np.ones(GENE_DIMENSIONS)
    attack_direction = attack_direction / np.linalg.norm(attack_direction)
    max_offset = sigma_multiplier * np.mean(honest_std)
    actual_offset = max_offset * 1.0 
    
    malicious_genes = []
    for _ in range(n_malicious):
        noise = rng.normal(0, 0.01, GENE_DIMENSIONS)
        malicious_gene = honest_mean + attack_direction * actual_offset + noise
        malicious_genes.append(malicious_gene)
    return np.array(malicious_genes)

def simulate_round(honest_nodes, malicious_nodes, f):
    all_nodes = np.vstack([honest_nodes, malicious_nodes])
    if len(malicious_nodes) == 0:
        return np.mean(all_nodes, axis=0), 1.0, []
        
    result, _, _ = real_multi_krum(all_nodes, f)
    if result is None: return None, 0.0, -1
    return result, 1.0, []

def calculate_drift(original, current, direction):
    drift = current - original
    mag = np.dot(drift, direction)
    orig_mag = np.linalg.norm(original)
    return abs(mag) / orig_mag if orig_mag > 0 else 0

def run_trial(f_ratio, seed):
    rng = np.random.default_rng(seed)
    n_mal = int(np.round(N_TOTAL * f_ratio))
    n_honest = N_TOTAL - n_mal
    
    honest_center = np.ones(GENE_DIMENSIONS)
    honest_std = HONEST_STD 
    current_consensus = honest_center.copy()
    original = honest_center.copy()
    direction = np.ones(GENE_DIMENSIONS) / np.linalg.norm(np.ones(GENE_DIMENSIONS))
    
    rounds_to_drift = MAX_ROUNDS
    drifted = False
    
    configured_f = int((N_TOTAL - 1) / 3) 

    for r in range(MAX_ROUNDS):
        honest = generate_honest_nodes(n_honest, current_consensus, honest_std, rng)
        mal = generate_malicious_nodes(n_mal, honest, 1.0, SIGMA_MULTIPLIER, rng)
        
        res, _, _ = simulate_round(honest, mal, configured_f)
        if res is None: continue
        
        current_consensus = 0.7 * current_consensus + 0.3 * res
        
        drift = calculate_drift(original, current_consensus, direction)
        if drift > DRIFT_THRESHOLD and not drifted:
            rounds_to_drift = r + 1
            drifted = True
            
    final_drift = calculate_drift(original, current_consensus, direction)
    return drifted, final_drift

def run_sweep():
    print("="*60)
    print(f"Byzantine Sensitivity Sweep (TrimmedMeanByz v19.0)")
    print(f"Storm Condition: StdDev={HONEST_STD}")
    print(f"Aggregator: Coordinate-wise Trimmed Mean (f={int((N_TOTAL-1)/3)})")
    print("="*60)
    
    ratios = np.arange(0.0, 0.40, 0.05) 
    csv_lines = ["attacker_ratio,fail_rate,avg_drift"]
    
    max_tolerant_ratio = 0.0
    
    fail_rates = []
    avg_drifts = []
    
    for ratio in ratios:
        drift_count = 0
        total_drift = 0.0
        
        for t in range(N_TRIALS):
            seed = int(ratio*1000) + t
            drifted, drift = run_trial(ratio, seed)
            if drifted: drift_count += 1
            total_drift += drift
            
        fail_rate = drift_count / N_TRIALS
        avg_drift = total_drift / N_TRIALS
        
        fail_rates.append(fail_rate)
        avg_drifts.append(avg_drift)
        
        print(f"Ratio {ratio:.2f} ({int(round(ratio*N_TOTAL))} nodes): Fail Rate {fail_rate*100:.0f}%, Avg Drift {avg_drift*100:.2f}%")
        csv_lines.append(f"{ratio:.2f},{fail_rate:.2f},{avg_drift:.4f}")
        
        if fail_rate <= 0.5: 
             max_tolerant_ratio = ratio
             
    # Save CSV
    with open("docs/RaaS_Data/robustness_sweep.csv", "w") as f:
        f.write("\n".join(csv_lines))
        
    # Tex summary
    tex = r"\begin{table}[h]" + "\n"
    tex += r"\centering" + "\n"
    tex += r"\begin{tabular}{lcc}" + "\n"
    tex += r"\toprule" + "\n"
    tex += r"Byzantine Ratio & Failure Rate & Mean Drift \\" + "\n"
    tex += r"\midrule" + "\n"
    for line in csv_lines[1:]:
        r, fr, d = line.split(',')
        tex += f"{float(r)*100:.0f}\\% & {float(fr)*100:.0f}\\% & {float(d)*100:.1f}\\% \\\\\n"
    tex += r"\bottomrule" + "\n"
    tex += r"\end{tabular}" + "\n"
    tex += r"\caption{Byzantine Tolerance under Storm Conditions}" + "\n"
    tex += r"\end{table}" + "\n"
    
    with open("docs/RaaS_Data/robustness_summary.tex", "w") as f:
        f.write(tex)

    print(f"\nMax Tolerated Ratio: {max_tolerant_ratio*100:.0f}%")
    
    # Generate Plot
    plt.figure(figsize=(10, 6))
    plt.plot(ratios*100, [fr*100 for fr in fail_rates], 'o-', label='Failure Rate (>5% Drift)')
    plt.plot(ratios*100, [d*100 for d in avg_drifts], 's--', label='Mean Drift %')
    plt.axvline(x=max_tolerant_ratio*100, color='g', linestyle=':', label=f'Max Tolerance ({max_tolerant_ratio*100:.0f}%)')
    plt.axhline(y=5.0, color='r', linestyle='--', alpha=0.5, label='5% Drift Threshold')
    
    plt.title(f'Byzantine Tolerance under Storm (StdDev={HONEST_STD:.2f})')
    plt.xlabel('Attacker Ratio (%)')
    plt.ylabel('Percentage (%)')
    plt.legend()
    plt.grid(True, alpha=0.3)
    
    plt.savefig('docs/images/tolerance_curve.png')
    print("Saved tolerance_curve.png")

if __name__ == "__main__":
    run_sweep()
