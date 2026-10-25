<p align="center">
  <img src="assets/banner.svg" alt="OU × CHAOS — Collaboration Capstone" width="100%" />
</p>

<p align="center">
  <img src="assets/lockup.svg" alt="CHAOS Industries × University of Oklahoma" width="720" />
</p>

<p align="center">
  <a href="https://github.com/Brasilius/OU-CHAOS-Mono-Repo/actions/workflows/ci.yml"><img src="https://github.com/Brasilius/OU-CHAOS-Mono-Repo/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://github.com/Brasilius/OU-CHAOS-Mono-Repo/actions/workflows/release.yml"><img src="https://github.com/Brasilius/OU-CHAOS-Mono-Repo/actions/workflows/release.yml/badge.svg" alt="Release" /></a>
  <img src="https://img.shields.io/github/last-commit/Brasilius/OU-CHAOS-Mono-Repo?color=841617&label=last%20commit&logo=github" alt="last commit" />
  <img src="https://img.shields.io/github/repo-size/Brasilius/OU-CHAOS-Mono-Repo?color=841617&label=repo%20size" alt="repo size" />
  <img src="https://img.shields.io/github/issues/Brasilius/OU-CHAOS-Mono-Repo?color=841617&label=issues" alt="issues" />
  <img src="https://img.shields.io/github/issues-pr/Brasilius/OU-CHAOS-Mono-Repo?color=841617&label=PRs" alt="PRs" />
  <img src="https://img.shields.io/badge/license-MIT-841617?logo=github" alt="license" />
  <img src="https://img.shields.io/badge/made%20with-%E2%9D%A4-841617?logo=ou" alt="made with love" />
</p>

> _"Creating the time to change the outcome."_
>
> A senior capstone collaboration between the **University of Oklahoma** and
> [**CHAOS Industries**](https://www.chaosinc.com) — turning mission
> engineering into defense-grade systems, built by students.

---

## 📑 Table of Contents

- [About](#-about)
- [Repository Structure](#-repository-structure)
- [Getting Started](#-getting-started)
- [CI/CD Pipelines](#-cicd-pipelines)
- [Roadmap](#-roadmap)
- [Contributors](#-contributors)
- [Acknowledgments](#-acknowledgments)

---

## 🎯 About

<img src="assets/divider.svg" width="100%" />

<!-- Describe the project mission, scope, and capstone deliverables here. -->

This monorepo hosts the shared codebase for the **OU × CHAOS Industries
collaboration capstone**: a year-long engineering partnership pairing OU
students with CHAOS Industries mentors to deliver real-world defense
technology.

| | |
|---|---|
| **Institution** | University of Oklahoma — Norman, OK |
| **Industry Partner** | [CHAOS Industries](https://www.chaosinc.com) — defense technology |
| **Duration** | 2025 – 2026 academic year |
| **Team** | _add team members here_ |

## 🗂 Repository Structure

<img src="assets/divider.svg" width="100%" />

```
OU-CHAOS-Mono-Repo/
├── .github/
│   └── workflows/        # CI/CD pipelines (ci.yml, release.yml)
├── assets/               # Branding, banners & animations
├── apps/                 # Deployable applications        (coming soon)
├── packages/             # Shared libraries               (coming soon)
├── docs/                 # Project documentation          (coming soon)
└── README.md
```

## 🚀 Getting Started

<img src="assets/divider.svg" width="100%" />

<!-- Add real setup instructions as the toolchain lands. -->

```bash
# Clone the monorepo
git clone https://github.com/Brasilius/OU-CHAOS-Mono-Repo.git
cd OU-CHAOS-Mono-Repo

# Install dependencies (placeholder)
# npm install

# Run the test suite (placeholder)
# npm test
```

## ⚙️ CI/CD Pipelines

<img src="assets/divider.svg" width="100%" />

Every push and pull request runs through the pipelines below. Status is
reported in the badges at the top of this README and on every commit.

```mermaid
---
config:
  theme: dark
---
flowchart LR
    A[Push / PR] --> B[Lint]
    B --> C[Build]
    C --> D[Unit Tests]
    D --> E{All green?}
    E -- yes --> F[Merge to main]
    E -- no --> G[Fail fast ❌]
    F --> H[Tag v*.*.*]
    H --> I[Release Pipeline]
    I --> J[Package Artifacts]
    J --> K[Deploy / Publish 🚀]

    style A fill:#841617,stroke:#ff6b5e,color:#fff
    style E fill:#841617,stroke:#ff6b5e,color:#fff
    style K fill:#1a7f37,stroke:#3fb950,color:#fff
    style G fill:#b62324,stroke:#ff6b5e,color:#fff
```

| Workflow | File | Trigger | Purpose |
|---|---|---|---|
| **CI** | [`ci.yml`](.github/workflows/ci.yml) | push / PR to `main` | Lint, build, test |
| **Release** | [`release.yml`](.github/workflows/release.yml) | version tags `v*` | Package & publish |

## 🗺 Roadmap

<img src="assets/divider.svg" width="100%" />

- [x] Stand up the monorepo
- [x] Wire up CI/CD pipelines
- [ ] Define project scope with CHAOS mentors
- [ ] First prototype milestone
- [ ] Mid-year demo
- [ ] Capstone showcase 🎓

## 👥 Contributors

<img src="assets/divider.svg" width="100%" />

<!-- Swap this table for an all-contributors widget once the team is set. -->

| Role | Name |
|---|---|
| Faculty Advisor | _TBD_ |
| Industry Mentor (CHAOS) | _TBD_ |
| Student Engineer | _add yourself!_ |

<a href="https://github.com/Brasilius/OU-CHAOS-Mono-Repo/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=Brasilius/OU-CHAOS-Mono-Repo&max=24&columns=12" alt="contributors" />
</a>

## 🙏 Acknowledgments

<img src="assets/divider.svg" width="100%" />

Sincere thanks to the engineers at [CHAOS Industries](https://www.chaosinc.com)
and the faculty and staff of the [University of Oklahoma](https://www.ou.edu)
for making this capstone possible.

---

<p align="center">
  <img src="assets/divider.svg" width="60%" />
</p>

<p align="center">
  <b>BOOMER</b> · <i>sooner</i> 🚀
</p>

<p align="center">
  <a href="https://www.ou.edu"><img src="https://img.shields.io/badge/University%20of%20Oklahoma-841617?style=for-the-badge&logo=wikiquote&logoColor=white" alt="OU" /></a>
  &nbsp;
  <a href="https://www.chaosinc.com"><img src="https://img.shields.io/badge/CHAOS%20Industries-0b0b0d?style=for-the-badge&logo=codeberg&logoColor=white" alt="CHAOS Industries" /></a>
</p>

<p align="center">
  <sub>Capstone • 2025–2026 • Norman, OK</sub>
</p>
