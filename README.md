#  Verificador de Modelos para CTL em Rust

Este repositório contém o código-fonte e o texto dissertativo referentes ao Trabalho de Conclusão de Curso (TCC) em Ciência da Computação no Instituto de Informática da Universidade Federal do Rio Grande do Sul (INF/UFRGS).

O projeto consiste no desenvolvimento do **Llull**, um verificador de modelos (*model checker*) composto por um motor explícito (`llull-labelling`) e um motor simbólico (`llull-BDD`) implementado em Rust para a lógica temporal *Computation Tree Logic* (CTL), avaliado comparativamente contra a ferramenta industrial NuSMV.

---

## 📁 Estrutura do Repositório

* `checker/`: Diretório contendo o código-fonte do verificador, suítes de testes e infraestrutura de automação.
    * `src/`: Implementação dos motores explícito e simbólico em Rust.
    * `benchmark/`: Scripts de automação (incluindo o pipeline de visualização e testes estatísticos).
    * `data/`: Intâncias 
    * `flake.nix` / `flake.lock`: Configurações declarativas do ambiente Nix para garantir reprodutibilidade.
* `doc/`: Código-fonte em LaTeX (`.tex`), figuras vetoriais e arquivos bibliográficos do texto da monografia.

---

## 🚀 Como Executar e Desenvolver (via Nix Flakes)

Para garantir que o pipeline experimental compile de maneira idêntica e previsível em qualquer máquina (mitigando problemas de versões globais de ferramentas, bibliotecas C ou compiladores), o projeto utiliza o ecossistema **Nix**.

### 1. Pré-requisitos
Certifique-se de ter o gerenciador de pacotes **Nix** instalado na sua máquina com o suporte a `flakes` e `nix-command` ativo. Utilize o comando correspondente ao seu sistema operacional:

* **Linux (Geral):**
    ```bash
    curl --proto '=https' --tlsv1.2 -L [https://nixos.org/nix/install](https://nixos.org/nix/install) | sh -s -- --daemon
    ```
* **WSL (Windows Subsystem for Linux):**
    ```bash
    curl --proto '=https' --tlsv1.2 -L [https://nixos.org/nix/install](https://nixos.org/nix/install) | sh -s -- --daemon
    ```
* **macOS:**
    ```bash
    curl --proto '=https' --tlsv1.2 -L [https://nixos.org/nix/install](https://nixos.org/nix/install) | sh
    ```

*(Se você já utiliza **NixOS**, seu sistema já atende nativamente a este requisito).*

### 2. Entrando no Ambiente de Desenvolvimento
Navegue até a pasta do verificador e inicialize o *development shell* declarado no arquivo `flake.nix`. O Nix irá baixar, isolar e disponibilizar automaticamente todas as ferramentas necessárias (compilador Rust/Cargo, interpretador Python, bibliotecas, NuSMV, etc.):

```bash
cd checker/
nix develop
```

### 3. Compilando e Executando
Uma vez dentro do ambiente isolado pelo nix develop, você pode interagir diretamente com o ecossistema do Rust:

- Compilar o projeto:

```bash
cargo build --release
```
- Executar a suíte de testes automatizados:

```bash
cargo test
```

- Verificar os comandos disoníves após a compilação:

```bash
checker --help
```

