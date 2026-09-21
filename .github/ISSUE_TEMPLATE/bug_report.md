---
name: "🐛 Bug Report"
about: Laporkan bug, anomali runtime, atau perilaku sistem yang tidak sesuai spesifikasi.
title: "[BUG]: <deskripsi singkat masalah>"
labels: ["type:bug", "status:triage-needed"]
assignees: ""
---

## 1. Environment & Metadata
- **Environment**: [Production / Staging / Local Dev]
- **App Version / Commit SHA**: [e.g., v2.4.1 / `a1b2c3d`]
- **Runtime / OS**: [e.g., Node.js 20.x / Python 3.11 / Ubuntu 22.04 LTS]
- **Browser / Client**: [e.g., Chrome, Postman, curl, iOS - jika relevan]
- **Impacted Service / Module**: [e.g., `auth`, `billing`, `sync-agent`]

---

## 2. Deskripsi Masalah & Dampak
<!-- Ringkasan singkat masalah dan seberapa parah pengaruhnya ke sistem atau pengguna -->

---

## 3. Langkah Mereproduksi Masalah (Steps to Reproduce)
1. Pergi ke `...`
2. Eksekusi endpoint/aksi `...` dengan payload/input `...`
3. Amati kegagalan sistem pada tahap `...`

---

## 4. Perilaku yang Diharapkan vs Perilaku Aktual
- **Expected Behavior**: [Perilaku yang seharusnya terjadi sesuai spesifikasi]
- **Actual Behavior**: [Perilaku nyata yang salah/gagal]

---

## 5. Minimal Reproducible Example (MRE)
<!-- Berikan cURL command, skrip test, atau cuplikan kode minimal untuk mereproduksi bug -->

```bash
curl -X POST http://localhost:8080/api/v1/... \
  -H "Authorization: Bearer <TOKEN>" \
  -H "Content-Type: application/json" \
  -d '{"key": "value"}'
```

---

## 6. Logs, Stack Trace, & Visual Evidence
<details>
<summary><b>Klik untuk melihat Stack Trace / Logs</b></summary>

```text
[ERROR] Timestamp [Service] Exception detail...
```
</details>

---

## 7. Hipotesis Akar Masalah (Root Cause Hypothesis)
<!-- Analisis awal teknis mengapa bug ini terjadi dan file/fungsi yang dicurigai -->
- [ ] Penyebab potensial 1
- [ ] Penyebab potensial 2
