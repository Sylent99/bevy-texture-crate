# Hosting the catalog (GitHub, free)

1. Create a **public** GitHub repository.
2. Upload: `ambientcg_catalog.py`, `HANDOVER.md`, `README.md`, the `bevy/` folder, and
   `.github/workflows/update-catalog.yml`.
3. In the repo: **Settings > Actions > General**, allow workflows, and under
   "Workflow permissions" choose **Read and write**.
4. **Actions** tab > "Update catalog" > **Run workflow**. After about a minute a
   `catalog/` folder appears with `index.csv` and one small CSV per category.
5. Your catalog index URL is:
   `https://raw.githubusercontent.com/<you>/<repo>/main/catalog/index.csv`
6. Put that URL into `HANDOVER.md` (replace `<CATALOG_INDEX_URL>`).

The workflow then refreshes the catalog every Monday.

How Claude uses it: a user pastes `HANDOVER.md` (which contains the index URL). Claude
fetches `index.csv`, picks the categories that fit the request, fetches only those small
files (their full URLs are listed in the index), and answers with a palette.
Nothing to download or upload.
