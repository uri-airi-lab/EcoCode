// EcoCode MetricRecord types

export interface GpuMetric {
  device_index: number
  gpu_usage: number // percentage 0-100
  gpu_energy: number // Joules
}

export interface SocketMetric {
  socket: number
  domain: string // "cpu", "dram", "igpu"
  energy: number // Joules
}

export interface MetricRecord {
  id: string
  run_id: string
  pid: number
  timestamp: number
  cpu_usage: number // percentage 0-100
  cpu_energy: number // Joules
  gpu_usage: number // percentage 0-100 (aggregate)
  gpu_energy: number // Joules (aggregate)
  mem_usage: number // percentage 0-100
  mem_energy: number // Joules
  igpu_usage: number // percentage 0-100
  igpu_energy: number // Joules
  gpu_details: GpuMetric[]
  socket_details: SocketMetric[]
}

export interface Run {
  id: string
  name: string
  timestamp: number
  status: 'running' | 'finished' | 'failed'
  collaboratorName?: string | null
  totalEnergy: number // Wh
  totalCpuEnergy: number // Wh
  totalGpuEnergy: number // Wh
  totalMemEnergy: number // Wh
  totalIgpuEnergy: number // Wh
  totalCoreHours: number
  avgCpuUsage: number // percentage
  avgGpuUsage: number // percentage
  avgMemUsage: number // percentage
  carbonFootprint: number // gCO2
  waterConsumption: number // mL
  duration: number // seconds
  command?: string | null
  systemInfo?: {
    cpuName: string
    cpuCores: number
    ramGb: number
    gpuCount: number
    gpuNames: string
  } | null
  carbonIntensityUsed?: number | null
  pueUsed?: number | null
  records: MetricRecord[]
}

// Constants for calculations
const CARBON_FACTOR = 0.475 // kgCO2/kWh (world average)
const WATER_FACTOR = 1.8 // L/kWh

// Helper to generate mock records for a run
function generateRunRecords(count: number, startTime: number): MetricRecord[] {
  const records: MetricRecord[] = []
  const timeStep = 60000 // 1 minute intervals in ms
  const gpuCount = 2 // simulate 2 GPUs

  for (let i = 0; i < count; i++) {
    const gpuDetails: GpuMetric[] = []
    let totalGpuUsage = 0
    let totalGpuEnergy = 0

    for (let g = 0; g < gpuCount; g++) {
      const usage = Math.random() > 0.3 ? 10 + Math.random() * 80 : 0
      const energy = Math.random() > 0.3 ? 30 + Math.random() * 200 : 0
      gpuDetails.push({ device_index: g, gpu_usage: usage, gpu_energy: energy })
      totalGpuUsage += usage
      totalGpuEnergy += energy
    }

    records.push({
      id: `record-${i}`,
      run_id: 'mock-run',
      pid: Math.floor(Math.random() * 65535),
      timestamp: startTime + i * timeStep,
      cpu_usage: 20 + Math.random() * 70,
      cpu_energy: 50 + Math.random() * 150,
      gpu_usage: totalGpuUsage / gpuCount,
      gpu_energy: totalGpuEnergy,
      mem_usage: 10 + Math.random() * 40,
      mem_energy: 5 + Math.random() * 15,
      igpu_usage: Math.random() > 0.8 ? 5 + Math.random() * 20 : 0,
      igpu_energy: Math.random() > 0.8 ? 2 + Math.random() * 10 : 0,
      gpu_details: gpuDetails,
      socket_details: [
        { socket: 0, domain: 'cpu', energy: 50 + Math.random() * 100 },
        { socket: 0, domain: 'dram', energy: 5 + Math.random() * 15 },
      ],
    })
  }

  return records
}

// Helper to calculate run metrics from records
function calculateRunMetrics(records: MetricRecord[]) {
  const totalCpuEnergy = records.reduce((sum, r) => sum + r.cpu_energy, 0)
  const totalGpuEnergy = records.reduce((sum, r) => sum + r.gpu_energy, 0)
  const totalMemEnergy = records.reduce((sum, r) => sum + r.mem_energy, 0)
  const totalIgpuEnergy = records.reduce((sum, r) => sum + r.igpu_energy, 0)
  const totalEnergy = totalCpuEnergy + totalGpuEnergy + totalMemEnergy + totalIgpuEnergy

  const avgCpuUsage =
    records.reduce((sum, r) => sum + r.cpu_usage, 0) / records.length
  const avgGpuUsage =
    records.reduce((sum, r) => sum + r.gpu_usage, 0) / records.length
  const avgMemUsage =
    records.reduce((sum, r) => sum + r.mem_usage, 0) / records.length

  // Convert Wh to kWh then apply factors
  const energyKwh = totalEnergy / 1000
  const carbonFootprint = energyKwh * CARBON_FACTOR * 1000 // gCO2
  const waterConsumption = energyKwh * WATER_FACTOR * 1000 // mL

  return {
    totalEnergy,
    totalCpuEnergy,
    totalGpuEnergy,
    totalMemEnergy,
    totalIgpuEnergy,
    avgCpuUsage,
    avgGpuUsage,
    avgMemUsage,
    carbonFootprint,
    waterConsumption,
    carbonIntensityUsed: CARBON_FACTOR * 1000,
    pueUsed: 1.0,
  }
}

// Generate 10 mock runs
export function generateMockRuns(): Run[] {
  const now = Date.now()
  const runs: Run[] = []

  for (let i = 0; i < 10; i++) {
    const recordCount = 20 + Math.floor(Math.random() * 40)
    const records = generateRunRecords(recordCount, now - (10 - i) * 86400000)

    const metrics = calculateRunMetrics(records)

    runs.push({
      id: `run-${i + 1}`,
      name: `Training Run ${i + 1}`,
      timestamp: now - (10 - i) * 86400000,
      status: i < 2 ? 'running' : i === 2 ? 'failed' : 'finished',
      collaboratorName: null,
      totalEnergy: metrics.totalEnergy,
      totalCpuEnergy: metrics.totalCpuEnergy,
      totalGpuEnergy: metrics.totalGpuEnergy,
      totalMemEnergy: metrics.totalMemEnergy,
      totalIgpuEnergy: metrics.totalIgpuEnergy,
      totalCoreHours: (metrics.totalEnergy / 1000) * 24, // Rough estimate
      avgCpuUsage: metrics.avgCpuUsage,
      avgGpuUsage: metrics.avgGpuUsage,
      avgMemUsage: metrics.avgMemUsage,
      carbonFootprint: metrics.carbonFootprint,
      waterConsumption: metrics.waterConsumption,
      duration: recordCount * 60, // 1 minute per record
      carbonIntensityUsed: metrics.carbonIntensityUsed,
      pueUsed: metrics.pueUsed,
      records,
    })
  }

  return runs.reverse()
}

// Get all runs with summary metrics (no records)
export function getRuns(): Omit<Run, 'records'>[] {
  const runs = generateMockRuns()
  return runs.map(({ records, ...rest }) => rest)
}

// Get a single run with all details
export function getRunById(id: string): Run | null {
  const runs = generateMockRuns()
  return runs.find((r) => r.id === id) || null
}

// Get comparison data between consecutive runs
export function getImprovements(): MetricRecord[] {
  const runs = getRuns()
  const improvements: MetricRecord[] = []

  for (let i = 1; i < runs.length; i++) {
    const prev = runs[i - 1]
    const curr = runs[i]

    improvements.push({
      id: `improvement-${i}`,
      run_id: curr.id,
      pid: 0,
      timestamp: curr.timestamp,
      cpu_usage:
        ((prev.avgCpuUsage - curr.avgCpuUsage) / prev.avgCpuUsage) * 100,
      cpu_energy: ((prev.totalEnergy - curr.totalEnergy) / prev.totalEnergy) * 100,
      gpu_usage:
        ((prev.avgGpuUsage - curr.avgGpuUsage) / prev.avgGpuUsage) * 100,
      gpu_energy:
        ((prev.carbonFootprint - curr.carbonFootprint) /
          prev.carbonFootprint) *
        100,
      mem_usage: ((prev.avgMemUsage - curr.avgMemUsage) / prev.avgMemUsage) * 100,
      mem_energy: ((prev.totalEnergy - curr.totalEnergy) / prev.totalEnergy) * 100,
      igpu_usage: 0,
      igpu_energy: 0,
      gpu_details: [],
      socket_details: [],
    })
  }

  return improvements
}

