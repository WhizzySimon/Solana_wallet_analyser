<template>
  <div class="container">
    <h1>Solana Wallet Analyzer</h1>

    <input v-model="walletAddress" placeholder="Enter wallet address" />
    <button @click="analyzeWallet">Analyze</button>

    <div v-if="loading">⏳ Analyzing...</div>

    <div v-if="trades.length > 0">
      <h2>Trades</h2>
      <table>
        <thead>
          <tr>
            <th>Time</th>
            <th>Sold</th>
            <th>Amount</th>
            <th>PNL (USD)</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(trade, i) in trades" :key="i">
            <td>{{ formatTime(trade.timestamp) }}</td>
            <td>{{ trade.sold_token }}</td>
            <td>{{ trade.sold_amount }}</td>
            <td :class="{ profit: trade.profit_loss > 0, loss: trade.profit_loss < 0 }">
              {{ trade.profit_loss.toFixed(2) }}
            </td>
          </tr>
        </tbody>
      </table>

      <h3>Total PnL: {{ totalPnl.toFixed(2) }} USD</h3>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue'

const walletAddress = ref('')
const trades = ref([])
const loading = ref(false)

const analyzeWallet = async () => {
  loading.value = true
  try {
    const response = await fetch('http://127.0.0.1:8080/api/pnl', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ wallet_address: walletAddress.value }),
    })

    if (!response.ok) {
      console.error('Backend error')
      return
    }

    const result = await response.json()
    trades.value = result.trades || []
    console.log("✅ Loaded trades:", trades.value)
  } catch (err) {
    console.error('Fetch failed:', err)
  } finally {
    loading.value = false
  }
}

const totalPnl = computed(() =>
  trades.value.reduce((sum, t) => sum + t.profit_loss, 0)
)

const formatTime = (ts) => new Date(ts * 1000).toLocaleString()
</script>

<style scoped>
.container {
  max-width: 800px;
  margin: auto;
  font-family: sans-serif;
  padding: 1rem;
}
input {
  width: 100%;
  padding: 0.5rem;
  margin-bottom: 0.5rem;
}
button {
  padding: 0.5rem 1rem;
}
table {
  width: 100%;
  margin-top: 1rem;
  border-collapse: collapse;
}
th,
td {
  border: 1px solid #ccc;
  padding: 0.5rem;
}
.profit {
  color: green;
}
.loss {
  color: red;
}
</style>
