![](assets/Mantis16.png)
# Mantis
*A power flow and linear sensitivity library written from scratch in Rust*

This project is still in its infancy.

**Desired features:**
- Reading power flow cases from industry standard file types like RAW
- Power flow solution using a variety of available methods
    - DC load flow
    - Fast decoupled
    - Gauss-Seidel
    - Newton-Raphson
- Onelines or bus view
- Linear sensitivity analysis like
    - Power transfer distribution factors
    - Line outage distribution factors
    - Voltage sensitivities
        - PV
        - QV
- Contingency analysis

## Installation

Build and run via cargo after creating a local copy of the repository

```bash
git clone https://github.com/SklorGalore/mantis.git
cd mantis
cargo build
cargo run
```
