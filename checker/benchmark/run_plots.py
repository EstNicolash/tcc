#!/usr/bin/env python3
"""
run_plots.py
================================================================================
Advanced Visualization Pipeline for Software Model Checking Benchmarks.

Features:
  - Automated Agnostic Dataset Discovery
  - Monolithic Data Type Harmonization & Standard Mapping
  - Precise Engine Calibration (Separating static and dynamic sifting algorithms)
  - Global Y-Axis Symmetrical Normalization for Side-by-Side Analysis
  - Unified Memory Normalization (Converts KB to MB globally)
  - Vectorial PDF Plotting with Strict Academic Layouts (SBMF/SBC compatible)
  - Non-parametric Kaplan-Meier Survival Analysis curves
"""

import os
import sys
import glob
import re
import numpy as np
import pandas as pd
import seaborn as sns
import matplotlib.pyplot as plt
from lifelines import KaplanMeierFitter
from matplotlib.ticker import LogFormatterMathtext


# Configurações estéticas acadêmicas globais (Padrão SBMF / SBC)
sns.set_theme(style="whitegrid", context="paper", font_scale=1.3)
plt.rcParams['figure.dpi'] = 300
plt.rcParams['font.family'] = 'serif'

def discover_latest_benchmark_csvs() -> dict:
    """Varre o diretório local de forma agnóstica descobrindo o CSV mais recente por problema."""
    base_dir = "."
    print(f"🔍 Varrendo '{os.path.abspath(base_dir)}' em busca das pastas de resultados mais recentes...")

    try:
        all_subdirs = [os.path.join(base_dir, d) for d in os.listdir(base_dir) if os.path.isdir(os.path.join(base_dir, d))]
    except Exception as e:
        print(f"❌ Erro ao acessar o diretório: {e}")
        return {}

    problem_groups = {}
    for d in all_subdirs:
        folder_name = os.path.basename(d)
        if folder_name in ["resultados_estatisticos_txt", "estatisticos", "txt", "__pycache__"] or folder_name.startswith("resultados_"):
            continue

        if "_results_" in folder_name:
            problem_name = folder_name.split("_results_")[0].strip().lower()
            if problem_name not in problem_groups:
                problem_groups[problem_name] = []
            problem_groups[problem_name].append(d)

    chosen_csvs = {}
    for problem_name, paths in problem_groups.items():
        paths.sort()
        latest_dir = paths[-1]

        csv_token = "synapses" if problem_name in ["synapse", "synapses"] else problem_name
        csv_files = glob.glob(os.path.join(latest_dir, f"benchmarks_{csv_token}.csv"))
        if not csv_files:
            csv_files = glob.glob(os.path.join(latest_dir, "*.csv"))

        if csv_files:
            chosen_csvs[problem_name] = csv_files[0]
            print(f"  -> {problem_name:14s}: Mais recente detectado -> {os.path.basename(latest_dir)}/{os.path.basename(csv_files[0])}")

    return chosen_csvs

def load_and_sanitize_data(file_path: str) -> pd.DataFrame:
    """Carrega os dados, padroniza as chaves e normaliza a RAM de KB para MB globalmente."""
    df = pd.read_csv(file_path)

    # Mapeamento estrito corrigido para isolar os algoritmos de sifting estático e dinâmico
    name_map = {
        'SSMV-BDD': 'llull-BDD',
        'NuSMV-Standard': 'NuSMV-sifting-static',
        'NuSMV-StaticImport': 'NuSMV-sifting-static',
        'NuSMV-DynamicSifting': 'NuSMV-sifting-dynamic',
        'SSMV-LABELLING-SCC': 'llull-labelling'
    }
    if 'Algorithm' in df.columns:
        df['Algorithm'] = df['Algorithm'].map(name_map).fillna(df['Algorithm'])

    # --- SALVA A HEURÍSTICA VIRGEM AQUI PARA EVITAR O CORTE DO SPLIT ---
    if 'Heuristic' in df.columns:
        df['Heuristic_Raw'] = df['Heuristic'].astype(str).str.lower()
        df['Heuristic'] = df['Heuristic'].astype(str).str.split(':').str[0].str.lower()

    numeric_cols = ['Total_MS', 'Compile_MS', 'Verify_MS', 'Static_Nodes', 'Verification_Nodes', 'States_Explored', 'Peak_Mem_KB']
    for col in numeric_cols:
        if col in df.columns:
            df[col] = pd.to_numeric(df[col], errors='coerce')

    if 'Peak_Mem_KB' in df.columns:
        df['Peak_Mem_MB'] = df['Peak_Mem_KB'] / 1024.0

    return df

def setup_output_directory(problem_name: str) -> str:
    """Cria a pasta limpa de destino para os gráficos daquele problema."""
    output_dir = f"resultados_{problem_name}"
    os.makedirs(output_dir, exist_ok=True)
    return output_dir

def plot_heuristic_comparison_curve(df: pd.DataFrame, target_col: str, label_y: str, problem_name: str, output_dir: str, clip_lower: float = None):
    """Gera gráficos comparando o impacto de heurísticas para cada algoritmo isolado com eixos Y sincronizados."""
    df_clean = df[df['Status'] == 'SUCCESS'].copy()
    if df_clean.empty or target_col not in df_clean.columns:
        return
    if clip_lower is not None:
        df_clean[target_col] = df_clean[target_col].clip(lower=clip_lower)

    # --- MAPEAMENTO SEGURO UTILIZANDO OS METADADOS RAW INDESTRUTÍVEIS ---
    orig_alg = df_clean['Algorithm'].copy()
    orig_heur_raw = df_clean['Heuristic_Raw'].copy() if 'Heuristic_Raw' in df_clean.columns else df_clean['Heuristic'].copy()

    df_clean['Heuristic_Clean'] = df_clean['Heuristic'].copy()

    # 1. Filtros estritos para o motor Dinâmico do NuSMV
    is_nusmv_dyn = (orig_alg == 'NuSMV-sifting-dynamic')
    df_clean.loc[is_nusmv_dyn & (orig_heur_raw.str.contains('dynamic')), 'Heuristic_Clean'] = 'sifting-dynamic'
    df_clean.loc[is_nusmv_dyn & (orig_heur_raw.str.contains('default')), 'Heuristic_Clean'] = 'default'

  # 2. Filtros estritos para o motor Estático do NuSMV (Injeção de Ordem Fixa)
    is_nusmv_stat = (orig_alg == 'NuSMV-sifting-static')
    df_clean.loc[is_nusmv_stat & (orig_heur_raw.str.contains('default')), 'Heuristic_Clean'] = 'default'
    df_clean.loc[is_nusmv_stat & (orig_heur_raw.str.contains('dynamic')), 'Heuristic_Clean'] = 'sifting-static'

    # 3. Correção para o llull-BDD: normaliza a string residual 'dynamic' para 'sifting'
    df_clean['Heuristic_Clean'] = df_clean['Heuristic_Clean'].replace('dynamic', 'sifting')

    # Unificamos a família de motores para o plot agrupar tudo no mesmo plano
    df_clean['Plot_Engine'] = df_clean['Algorithm'].copy()

    # Unificamos a família de motores para o plot agrupar tudo no mesmo plano
    df_clean['Plot_Engine'] = df_clean['Algorithm'].copy()
    df_clean.loc[df_clean['Algorithm'].str.startswith('NuSMV'), 'Plot_Engine'] = 'NuSMV'
    # ---------------------------------------------------------------------

    # --- CÁLCULO DOS LIMITES GLOBAIS DA ESCALA Y ---
    y_min = df_clean[target_col].min()
    y_max = df_clean[target_col].max()

    if y_min <= 0 or np.isnan(y_min):
        y_min = 0.1 if clip_lower is None else clip_lower
    y_limit_min = y_min * 0.8
    y_limit_max = y_max * 1.5
    # ---------------------------------------------------------------------

    for algoritmo in df_clean['Plot_Engine'].unique():
        df_alg = df_clean[df_clean['Plot_Engine'] == algoritmo]
        if len(df_alg['Heuristic_Clean'].unique()) <= 1:
            continue

        plt.figure(figsize=(7.5, 5))
        ax = sns.lineplot(
            data=df_alg, x='N', y=target_col, hue='Heuristic_Clean', style='Heuristic_Clean',
            markers=True, dashes=False, linewidth=2.5, errorbar=('ci', 95)
        )
        plt.yscale('log')
        ax.yaxis.set_major_formatter(LogFormatterMathtext())

        # Sincronização simétrica dos eixos Y
        ax.set_ylim(bottom=y_limit_min, top=y_limit_max)

        metric_name = "Verificação" if "verify" in target_col.lower() else ("Compilação" if "compile" in target_col.lower() else ("Memória RAM" if "mem" in target_col.lower() else "Nós BDD"))
        display_name = "NuSMV (Sifting)" if algoritmo == 'NuSMV' else algoritmo

        plt.title(f"Impacto de Heurísticas no Tempo de {metric_name}: {display_name} ({problem_name.upper()})", fontsize=12, pad=12)
        plt.xlabel("Parâmetro de Escala (N)", fontsize=11)
        plt.ylabel(f"{label_y} (Escala Log)", fontsize=11)
        plt.xticks(sorted(df_alg['N'].unique()))
        plt.legend(title="Estratégia / Heurística", frameon=True, facecolor='white', edgecolor='0.8', loc='upper left')

        sns.despine(left=True, bottom=True)
        plt.tight_layout()

        file_tail = algoritmo.lower().replace('-', '_')
        plt.savefig(os.path.join(output_dir, f"grafico_comp_heuristica_{file_tail}_{problem_name}_{target_col.lower()}.pdf"), bbox_inches='tight')
        plt.close()

def plot_scalability_curve(df: pd.DataFrame, target_col: str, label_y: str, problem_name: str, output_dir: str, clip_lower: float = None):
    """Gera curvas de escalabilidade comparando os motores para cada heurística estável."""
    df_clean = df[df['Status'] == 'SUCCESS'].copy()
    if df_clean.empty or target_col not in df_clean.columns:
        return
    if clip_lower is not None:
        df_clean[target_col] = df_clean[target_col].clip(lower=clip_lower)
    if target_col in ['Verification_Nodes', 'Static_Nodes']:
        df_clean = df_clean[df_clean['Algorithm'] != 'llull-labelling']

    for heuristica in df_clean['Heuristic'].unique():
        df_heur = df_clean[df_clean['Heuristic'] == heuristica]
        plt.figure(figsize=(7.5, 5))
        ax = sns.lineplot(
            data=df_heur, x='N', y=target_col, hue='Algorithm', style='Algorithm',
            markers=True, dashes=False, linewidth=2.5, errorbar=('ci', 95)
        )
        plt.yscale('log')
        ax.yaxis.set_major_formatter(LogFormatterMathtext())
        plt.title(f"{label_y} - {problem_name.upper()} ({heuristica.upper()})", fontsize=12, pad=12)
        plt.xlabel("Parâmetro de Escala (N)", fontsize=11)
        plt.ylabel(f"{label_y} (Escala Log)", fontsize=11)
        plt.xticks(sorted(df_heur['N'].unique()))
        plt.legend(title="Algoritmo / Engine", frameon=True, facecolor='white', edgecolor='0.8', loc='upper left')

        sns.despine(left=True, bottom=True)
        plt.tight_layout()
        plt.savefig(os.path.join(output_dir, f"grafico_log_{target_col.lower()}_{problem_name}_{heuristica.lower()}.pdf"), bbox_inches='tight')
        plt.close()

def run_survival_analysis(df: pd.DataFrame, heuristic: str, engines: list, problem_name: str, output_dir: str, timeout_value: float = 60000.0):
    """Gera gráficos de estimadores de Kaplan-Meier para os timeouts estruturais."""
    df_heur = df[df['Heuristic'] == heuristic].copy()
    if df_heur.empty:
        return
    df_heur['Tempo_Tratado'] = df_heur['Total_MS'].fillna(timeout_value)
    df_heur['Observed'] = np.where(df_heur['Status'] == 'SUCCESS', 1, 0)

    plt.figure(figsize=(8, 5))
    kmf = KaplanMeierFitter()
    for engine in engines:
        df_eng = df_heur[df_heur['Algorithm'] == engine]
        if len(df_eng) > 0:
            kmf.fit(durations=df_eng['Tempo_Tratado'], event_observed=df_eng['Observed'], label=engine)
            kmf.plot_survival_function(ci_show=False, linewidth=2.5)

    if heuristic == 'dynamic':
        heuristic = 'sifting'

    plt.title(f"Curva de Sobrevivência (Kaplan-Meier) - {problem_name.upper()} ({heuristic.upper()})", fontsize=12, pad=12)
    plt.xlabel("Tempo de Execução (ms)", fontsize=11)
    plt.ylabel("Proporção de Instâncias Não Resolvidas", fontsize=11)
    plt.xscale('log')
    plt.grid(True, which='both', ls='--', alpha=0.5)
    plt.tight_layout()
    plt.savefig(os.path.join(output_dir, f"grafico_survival_kaplan_meier_{problem_name}_{heuristic}.pdf"), bbox_inches='tight')
    plt.close()

# ==================== PIPELINE BATCH PRINCIPAL ====================
if __name__ == "__main__":
    target_csvs = discover_latest_benchmark_csvs()

    if not target_csvs:
        print("❌ Erro: Nenhuma pasta estruturada de benchmarks encontrada.")
        sys.exit(1)

    print(f"\n📈 Iniciando lote de geração de gráficos para {len(target_csvs)} problemas...")

    for problem_name, csv_path in target_csvs.items():
        print(f"\n➔ Renderizando plots vetoriais para: '{problem_name.upper()}'")

        OUT_DIR = setup_output_directory(problem_name)
        raw_data = load_and_sanitize_data(csv_path)

        # 1. Curvas de escalabilidade entre motores
        plot_scalability_curve(raw_data, 'Total_MS', 'Tempo Total de Execução (ms)', problem_name, OUT_DIR)
        plot_scalability_curve(raw_data, 'Verify_MS', 'Tempo de Verificação (ms)', problem_name, OUT_DIR, clip_lower=0.1)
        plot_scalability_curve(raw_data, 'Compile_MS', 'Tempo de Compilação (ms)', problem_name, OUT_DIR, clip_lower=0.1)
        plot_scalability_curve(raw_data, 'Peak_Mem_MB', 'Consumo de Memória RAM (MB)', problem_name, OUT_DIR)
        plot_scalability_curve(raw_data, 'Verification_Nodes', 'Quantidade de Nós BDD', problem_name, OUT_DIR, clip_lower=1.0)

        # 2. Curvas de sobrevivência de Kaplan-Meier
        run_survival_analysis(raw_data, 'default', ['llull-BDD', 'NuSMV-sifting-static'], problem_name, OUT_DIR)
        run_survival_analysis(raw_data, 'force',   ['llull-BDD', 'NuSMV-sifting-static'], problem_name, OUT_DIR)
        run_survival_analysis(raw_data, 'dynamic', ['llull-BDD', 'NuSMV-sifting-static', 'NuSMV-sifting-dynamic'], problem_name, OUT_DIR)

        # 3. Cruzamento de Heurísticas por motor (Legendas Higienizadas e Eixos Sincronizados)
        plot_heuristic_comparison_curve(raw_data, 'Total_MS', 'Tempo Total de Execução (ms)', problem_name, OUT_DIR)
        plot_heuristic_comparison_curve(raw_data, 'Verify_MS', 'Tempo de Verificação (ms)', problem_name, OUT_DIR, clip_lower=0.1)
        plot_heuristic_comparison_curve(raw_data, 'Compile_MS', 'Tempo de Compilação (ms)', problem_name, OUT_DIR, clip_lower=0.1)
        plot_heuristic_comparison_curve(raw_data, 'Peak_Mem_MB', 'Consumo de Memória RAM (MB)', problem_name, OUT_DIR, clip_lower=0.1)
        plot_heuristic_comparison_curve(raw_data, 'Verification_Nodes', 'Quantidade de Nós BDD', problem_name, OUT_DIR, clip_lower=0.1)

        print(f"  -> Sucesso: Gráficos gravados em 'resultados_{problem_name}/'")

    print("\n🎉 Concluído com sucesso! Todas as instâncias mais recentes foram processadas.")
