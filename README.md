# HackerOne Asset Fetcher (Rust)

Bu araç, HackerOne Hacker API üzerinden erişebildiğiniz programların `structured_scopes` (asset) listesini çeker.

## Gereksinimler

- Rust toolchain
- HackerOne API kimlik bilgileri (`username` + `token`)

## Kurulum

```bash
cargo build --release
```

## Kullanım

### 1) Ortam değişkenleri ile

```bash
export H1_USERNAME="kullanici_adiniz"
export H1_TOKEN="api_tokeniniz"
cargo run -- --output table
```

### 2) Parametre ile

```bash
cargo run -- --username "kullanici_adiniz" --token "api_tokeniniz" --output json
```

### Belirli program(lar) için asset çekme

```bash
cargo run -- --program hackerone --program uber --output table
```

## Çıktı formatları

- `--output table`: Terminal tablosu
- `--output json`: JSON çıktısı

## Notlar

- Araç, API pagination (`links.next`) bilgisini takip eder.
- `--program` verilmezse önce erişilebilir programları listeler, ardından her program için `structured_scopes` endpoint'ini çağırır.
