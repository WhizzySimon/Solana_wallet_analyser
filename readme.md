# 🔍 Solana Wallet Analyzer

Analyze trading activity of any Solana wallet. The backend detects token swaps, resolves token names, enriches with USD prices, and calculates PnL using FIFO or LIFO. The frontend displays results in a clean UI.

## ✨ Features

- Fetch wallet transactions via Helius
- Detect and normalize token swaps
- Resolve token names from Jupiter and cache
- Enrich swaps with USD prices from BirdEye, Jupiter, or Binance
- Calculate per-token PnL using FIFO or LIFO
- REST API (Axum) + Vue 3 frontend

---

## ⚙️ Backend Setup (Rust)

**Requirements**: Rust 1.70+, Helius & BirdEye API keys

```bash
cd backend
cp .env.example .env         # Add your API keys
cargo build
cargo run -- <WALLET_ADDRESS>  # Or run as HTTP server
```

Example `.env`:
```
helius_api_key=YOUR_KEY
birdeye_api_key=YOUR_KEY
```

Optional `config.toml`:
```toml
[config]
fifo = true
use_token_cache = true
use_cached_priced_swaps = false
write_cache_files = true
```

---

## 🖥 Frontend Setup (Vue 3)

**Requirements**: Node.js 18+, pnpm

```bash
cd frontend
pnpm install
pnpm run dev
```

---

## 📄 License

MIT License
