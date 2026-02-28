import axios from 'axios'

const api = axios.create({
  baseURL: '/',
  timeout: 600_000, // 10 min for long inference
  headers: { 'Content-Type': 'application/json' },
})

// Attach API key if configured
api.interceptors.request.use((config) => {
  const key = localStorage.getItem('api_key')
  if (key) {
    config.headers.Authorization = `Bearer ${key}`
  }
  return config
})

export default api
