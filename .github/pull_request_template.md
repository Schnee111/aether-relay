## 📌 Context & Problem Statement
<!-- 
Jelaskan latar belakang masalah, kebutuhan bisnis, atau tiket yang diselesaikan.
Contoh: Fixes #123 / Resolves ENG-450.
-->
- **Ticket / Issue Link:** 
- **Problem Statement:** 

---

## 📝 Summary of Changes
<!-- Rangkuman poin-poin perubahan apa saja yang dibuat -->
- 

---

## 🛠️ Technical Approach & Design Decisions
<!-- 
Jelaskan arsitektur/pendekatan teknis yang diambil, trade-offs, atau alasan memilih solusi ini.
Apakah ada breaking changes, skema DB baru, atau penambahan env variables?
-->
- **Approach:** 
- **DB Migrations / Schema Changes:** [None / Yes (Detail below)]
- **New Environment Variables:** [None / Yes (Detail below)]

---

## 🧪 Verification & Evidence (Proof of Work)
<!-- 
Sertakan bukti konkret pengujian lokal/staging:
- CLI command / test runner outputs (pytest, vitest, go test)
- cURL request & responses
- Screenshots / Screen recording (Before & After) untuk UI
-->

### 1. Test Suite Results
```bash
# Tempel output test di sini
```

### 2. Manual Verification / Smoke Test
```bash
# Tempel perintah cURL / CLI smoke test di sini
```

### 3. UI Changes (Optional)
| Before | After |
| :--- | :--- |
| <!-- Gambar/GIF Before --> | <!-- Gambar/GIF After --> |

---

## ⚠️ Risk Assessment, Blast Radius & Rollback

- **Risk Level:** `[ ] Low` | `[ ] Medium` | `[ ] High` | `[ ] Critical`
- **Blast Radius:** <!-- Layanan/modul apa saja yang berpotensi terpengaruh? -->
- **Rollback Strategy:** 
  - [ ] Revert PR commit (`git revert -m 1 <commit-hash>`)
  - [ ] Rollback database migration: `npm run db:migrate:down` / `alembic downgrade -1`
  - [ ] Toggle feature flag: `<FLAG_KEY>`

---

## ✅ Pre-merge Checklist (Definition of Done)

- [ ] Judul PR telah mengikuti standar **Conventional Commits & Ticket ID** (maksimal 72 karakter).
- [ ] Kode telah melalui pengujian lokal dan seluruh *unit/integration test* berstatus lolos.
- [ ] Linter dan static type checking tidak menghasilkan error (`npm run lint`, `tsc --noEmit`, `ruff check`).
- [ ] Tidak menyertakan kredensial rahasia, token API, file `.env`, atau artefak build sementara.
- [ ] Dokumentasi kode / API schema (OpenAPI/Swagger/README) telah diperbarui jika ada perubahan antarmuka.
- [ ] Kompatibel untuk *zero-downtime deployment* (migrasi DB *backward compatible*).
