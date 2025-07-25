<template>
  <div class="app">
    <h1>Solana Wallet Analyzer</h1>

    <form @submit.prevent="fetchPnl">
      <input
        type="text"
        v-model="walletAddress"
        placeholder="Enter wallet address"
        required
      />
      <button type="submit">Analyze</button>
    </form>

    <div v-if="loading">🔄 Loading...</div>
    <div v-if="error" class="error">❌ {{ error }}</div>

    <table v-if="trades.length">
      <thead>
        <tr>
          <th>Timestamp</th>
          <th>Buy</th>
          <th>Sell</th>
          <th>PNL (USD)</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="trade in trades" :key="trade.signature">
          <td>{{ formatDate(trade.timestamp) }}</td>
          <td>{{ trade.buy_token_name }}: {{ trade.buy_amount }}</td>
          <td>{{ trade.sell_token_name }}: {{ trade.sell_amount }}</td>
          <td>{{ trade.pnl_usd?.toFixed(2) }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'

const walletAddress = ref('')
const trades = ref<any[]>([])
const loading = ref(false)
const error = ref('')

const fetchPnl = async () => {
  loading.value = true
  error.value = ''
  trades.value = []

  try {
    const response = await fetch('http://127.0.0.1:8080/api/pnl', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ wallet_address: walletAddress.value }),
    })

    if (!response.ok) throw new Error(`Status ${response.status}`)
    const data = await response.json()
    trades.value = data
  } catch (err: any) {
    error.value = err.message || 'Failed to fetch PNL'
  } finally {
    loading.value = false
  }
}

const formatDate = (ts: number) =>
  new Date(ts * 1000).toLocaleString()
</script>

<style scoped>
.app {
  padding: 2rem;
  font-family: Arial, sans-serif;
}

input {
  padding: 0.5rem;
  width: 300px;
  margin-right: 1rem;
}

button {
  padding: 0.5rem 1rem;
}

table {
  margin-top: 2rem;
  border-collapse: collapse;
  width: 100%;
}

th, td {
  border: 1px solid #ccc;
  padding: 0.5rem;
  text-align: left;
}

.error {
  color: red;
  margin-top: 1rem;
}
</style>
