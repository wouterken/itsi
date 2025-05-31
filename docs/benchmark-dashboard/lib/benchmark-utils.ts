// This file would contain utility functions for processing benchmark data
// For example, parsing JSON files, aggregating results, etc.

export type BenchmarkResult = {
  server: string
  test_case: string
  threads: number
  workers: number
  http2: boolean
  concurrency: number
  rss_mb?: number
  results: {
    successRate: number
    total: number
    slowest: number
    fastest: number
    average: number
    requestsPerSec: number
    totalData: number
    sizePerRequest: number
    sizePerSec: number
    errorDistribution: Record<string, number>
    p95_latency: number
  }
  timestamp?: string
}

/**
 * In a real implementation, this function would:
 * 1. Scan the directory structure
 * 2. Read and parse the JSON files
 * 3. Return the aggregated data
 */
export async function loadBenchmarkData(): Promise<BenchmarkResult[]> {
  // This is a placeholder for the actual implementation
  return []
}

/**
 * Calculate summary statistics for benchmark results
 */
export function calculateStats(data: BenchmarkResult[], metric: keyof BenchmarkResult["results"]) {
  if (data.length === 0) return null

  const values = data.map((item) => item.results[metric] as number)

  return {
    count: values.length,
    min: Math.min(...values),
    max: Math.max(...values),
    avg: values.reduce((sum, val) => sum + val, 0) / values.length,
    median: values.sort((a, b) => a - b)[Math.floor(values.length / 2)],
  }
}
