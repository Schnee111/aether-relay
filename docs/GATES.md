# Quality Gates Specification (DoR & DoD) — AetherRelay

Dokumen ini mendefinisikan kriteria gerbang kualitas biner (*quality gates*) yang wajib dipatuhi oleh seluruh developer dan autonomous AI agents sebelum memulai atau menyelesaikan pekerjaan pada repositori AetherRelay.

---

## 1. Definition of Ready (DoR) — Gerbang Masuk
*Sebuah task, issue, atau branch fitur DILARANG dikerjakan sebelum memenuhi seluruh kriteria berikut:*

1. **Problem Clarity & Boundary**: Masalah teknis, kebutuhan bisnis, atau latar belakang bug dijelaskan tanpa kalimat ambigu. Batasan ruang lingkup (in-scope vs out-of-scope) dinyatakan eksplisit.
2. **Binary Acceptance Criteria (AC)**: Seluruh kriteria penerimaan dinyatakan dalam bentuk checklist biner yang dapat diuji (lulus atau gagal secara objektif, bukan interpretasi rasa).
3. **Contract & Signature Specification**: Jika task menyangkut endpoint baru atau perubahan skema, kontrak request/response JSON atau DDL database sudah didefinisikan terlebih dahulu.
4. **Architectural Alignment**: Perubahan struktural telah memiliki berkas ADR terkait di folder `docs/adr/`.
5. **Dependencies Resolved**: Seluruh dependensi (kredensial mock, library eksternal, atau branch pendahulu untuk Stacked PRs) sudah tersedia.

Jika salah satu poin di atas belum terpenuhi, issue wajib diberi label `status:blocked` dan pekerjaan tidak boleh dimulai.

---

## 2. Definition of Done (DoD) — Gerbang Keluar
*Sebuah Pull Request DILARANG di-merge ke branch `main` sebelum memenuhi seluruh kriteria berikut:*

1. **Deterministic Test Verification**:
   - Seluruh unit tests, integration tests, dan test kasus konkurensi lulus 100% di local dan CI (`pnpm test`).
   - Tidak ada test yang di-skip atau di-mock secara tidak wajar.
2. **Static Code Quality & Type Safety**:
   - `pnpm tsc --noEmit` lulus tanpa error atau warning pada TypeScript strict mode.
   - Linter (`eslint` / `biome` / `ruff`) lulus tanpa ada rule yang di-disable tanpa justifikasi ADR.
3. **Commit & PR Formatting Compliance**:
   - Seluruh commit mengikuti format Conventional Commits 1.0.0 dengan body 3W (Why, What, Side-effects) untuk commit non-trivial.
   - Judul PR mengikuti format `<type>(<scope>): [<TICKET>] <summary>`.
   - Deskripsi PR terisi lengkap sesuai template `.github/pull_request_template.md` disertai lampiran bukti empiris (*proof of work*).
4. **Independent Peer / Sentinel Review**:
   - Minimal 1 approval resmi (`reviewDecision == 'APPROVED'`).
   - Zero unresolved threads pada komentar berkategori `issue [blocking]` atau `todo [blocking]`.
5. **Documentation & Spec Synchronization**:
   - Berkas `README.md`, `docs/SPEC.md`, atau skema OpenAPI telah disinkronkan dengan perubahan antarmuka atau variabel lingkungan baru.
6. **Zero-Downtime & Backward Compatibility**:
   - Perubahan skema database SQLite bersifat non-destructive dan kompatibel dengan rollback atomik (`git revert -m 1`).
