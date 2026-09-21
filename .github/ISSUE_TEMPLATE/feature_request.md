---
name: "🚀 Feature Request / RFC"
about: Usulkan fitur baru, peningkatan kapabilitas produk, atau Request for Comments (RFC) arsitektur.
title: "[FEAT/RFC]: <judul fitur atau usulan teknis>"
labels: ["type:feature", "status:triage-needed"]
assignees: ""
---

## 1. Problem Statement & User Pain Point
<!-- Masalah apa yang sedang dihadapi user atau sistem saat ini? Mengapa solusi eksisting tidak memadai? -->

---

## 2. Nilai Bisnis & Pengguna (User Value & Impact)
<!-- Siapa yang diuntungkan? Metrik atau KPI apa yang meningkat setelah fitur ini dirilis? -->
- **Target Persona**: [e.g., End User, Developer, Internal Ops]
- **Target Metric**: [e.g., Mengurangi latency sebesar 30%, Eliminasi manual sync]

---

## 3. Solusi yang Diusulkan (Proposed Solution)
<!-- Deskripsi fungsional dan teknis tingkat tinggi mengenai solusi yang ingin dibangun -->

---

## 4. Pertimbangan Desain Teknis (Technical Design Considerations)

### A. API / Contract Changes
```json
{
  "endpoint": "POST /v1/...",
  "request_body": {}
}
```

### B. Database & Schema Migrations
- [ ] Perlu tabel/kolom baru:
- [ ] Migration strategy: Zero-downtime / backward compatible

### C. Security & Permission
- [ ] RBAC / Auth requirements:
- [ ] Rate limiting:

### D. Performance, Scalability & SLAs
- [ ] Latency target (P95/P99):
- [ ] Caching strategy:

---

## 5. Alternatif yang Dipertimbangkan (Alternatives Considered)
| Alternatif | Kelebihan | Kekurangan | Alasan Ditolak / Diterima |
| :--- | :--- | :--- | :--- |
| **Opsi 1 (Terpilih)** | ... | ... | ... |
| **Opsi 2** | ... | ... | ... |

---

## 6. Definition of Done (DoD) Checklist
- [ ] Schema database termigrasi tanpa downtime.
- [ ] Core business logic & edge cases teruji via Unit Tests.
- [ ] Endpoint teruji via E2E / Integration Tests.
- [ ] Dokumentasi API (OpenAPI / README) terbarui.
- [ ] Log agregasi dan alert dashboard diatur di monitoring stack.
