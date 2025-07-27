<template>
  <div class="container">
    <h1>Solana Wallet Analyzer</h1>

    <input v-model="walletAddress" placeholder="Enter wallet address" />
    <button @click="analyzeWallet">Analyze</button>

    <div v-if="loading">⏳ Analyzing...</div>

    <div v-if="tokenPnls.length > 0">
      <label><input type="checkbox" v-model="excludeAirdrops" /> Exclude airdrops</label>

      <h2>Token PnL for the last 30 days</h2>
      <table>
        <thead>
          <tr>
            <th>Token</th>
            <th>Buys</th>
            <th>Buy USD</th>
            <th>Sells</th>
            <th>Sell USD</th>
            <th>Total PnL (USD)</th>
            <th>Airdrop?</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(t, i) in filteredPnls" :key="i">
            <td>{{ t.token }}</td>
            <td>{{ t.total_bought.toFixed(2) }}</td>
            <td>{{ getBuyUsd(t).toFixed(2) }}</td>
            <td>{{ t.total_sold.toFixed(2) }}</td>
            <td>{{ getSellUsd(t).toFixed(2) }}</td>
            <td :class="{ profit: t.realized_pnl > 0, loss: t.realized_pnl < 0 }">
              {{ t.realized_pnl.toFixed(2) }}
            </td>
            <td>{{ t.buys.length === 0 ? '✅' : '' }}</td>
          </tr>
        </tbody>
      </table>

      <h3>Total Realized PnL (30 days): {{ totalFilteredPnl.toFixed(2) }} USD</h3>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue'

const walletAddress = ref('')
const tokenPnls = ref([])
const loading = ref(false)
const excludeAirdrops = ref(false)

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
    tokenPnls.value = result.trades || []
    console.log("✅ Loaded tokens:", tokenPnls.value)
  } catch (err) {
    console.error('Fetch failed:', err)
  } finally {
    loading.value = false
  }
}

const filteredPnls = computed(() =>
  excludeAirdrops.value
    ? tokenPnls.value.filter(t => t.buys.length > 0)
    : tokenPnls.value
)

const totalFilteredPnl = computed(() =>
  filteredPnls.value.reduce((sum, t) => sum + t.realized_pnl, 0)
)

const getBuyUsd = (t) =>
  t.buys.reduce((sum, b) => sum + b.cost_usd, 0)

const getSellUsd = (t) =>
  t.sells.reduce((sum, s) => sum + s.proceeds_usd, 0)
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
