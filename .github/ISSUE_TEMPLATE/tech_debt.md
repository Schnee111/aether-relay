---
name: "🛠️ Task / Tech Debt / Chore"
about: Pekerjaan refactoring, upgrade dependensi, perbaikan performa, infrastruktur, atau utang teknis.
title: "[CHORE/DEBT]: <deskripsi refactoring atau task>"
labels: ["type:tech-debt", "status:triage-needed"]
assignees: ""
---

## 1. Konteks & Rasional (Context & Rationale)
<!-- Mengapa utang teknis ini harus diselesaikan sekarang? Apa risiko bila ditunda? -->

---

## 2. Cakupan Pekerjaan & Modul Terdampak (Scope of Work)
- **Komponen**: [e.g., `packages/database`, `infra/docker`, `src/utils/auth`]
- **Rincian Task**:
  - [ ] Task 1
  - [ ] Task 2
  - [ ] Task 3

---

## 3. Analisis Risiko & Blast Radius
- **Tingkat Risiko**: [Low / Medium / High]
- **Potensi Dampak Samping**: <!-- Modul atau flow apa yang bisa terdampak? -->
- **Mitigasi**: <!-- Rollback plan / canary / feature flag -->

---

## 4. Kriteria Penerimaan & Rencana Verifikasi (Acceptance Criteria)
- [ ] Tidak ada breaking changes pada consumer module.
- [ ] Test coverage tetap stabil / tidak ada test regresi.
- [ ] Memory/CPU footprint di baseline load terkontrol.
- [ ] CI/CD pipeline lulus tanpa warning baru.
