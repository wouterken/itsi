"use client"

import { useState, useEffect } from "react"
import { BenchmarkDashboard } from "@/components/benchmark-dashboard"
import { LoadingSpinner } from "@/components/ui/loading-spinner"

// Updated sample data with the new group hierarchy and longer version strings
const sampleHierarchicalData = [
  {
    cpu: "apple_m1_pro",
    groups: [
      {
        group: "rack",
        tests: [
          {
            test: "chunked",
            servers: [
              {
                server: "agoo",
                results: [
                  {
                    server: "agoo",
                    test_case: "chunked",
                    version: "Agoo v1.2.3 (Ruby 3.2.0) with experimental HTTP/2 support",
                    threads: 1,
                    workers: 1,
                    http2: false,
                    concurrency: 10,
                    rss_mb: 1.77,
                    results: {
                      successRate: 1.0,
                      total: 20000.0,
                      slowest: 5000000.0,
                      fastest: 200000.0,
                      average: 2500000.0,
                      requestsPerSec: 6500.0,
                      totalData: 0,
                      sizePerRequest: 0.0,
                      sizePerSec: 0.0,
                      errorDistribution: {},
                      p95_latency: 4.8,
                    },
                    timestamp: "2025-05-24T10:41:17Z",
                  },
                  {
                    server: "agoo",
                    test_case: "chunked",
                    version: "Agoo v1.2.3 (Ruby 3.2.0) with experimental HTTP/2 support",
                    threads: 1,
                    workers: 1,
                    http2: false,
                    concurrency: 50,
                    rss_mb: 1.77,
                    results: {
                      successRate: 0.95,
                      total: 28000.0,
                      slowest: 12000000.0,
                      fastest: 400000.0,
                      average: 6000000.0,
                      requestsPerSec: 9500.0,
                      totalData: 0,
                      sizePerRequest: 0.0,
                      sizePerSec: 0.0,
                      errorDistribution: {
                        timeout: 50,
                      },
                      p95_latency: 10.2,
                    },
                    timestamp: "2025-05-24T10:41:17Z",
                  },
                ],
              },
              {
                server: "puma",
                results: [
                  {
                    server: "puma",
                    test_case: "chunked",
                    version:
                      "Puma version 6.6.0+h2o version 2.3.0-DEV@87e2aa634 (Ruby 3.2.2) with HTTP/2 support enabled",
                    threads: 1,
                    workers: 1,
                    http2: true,
                    concurrency: 10,
                    results: {
                      successRate: 1.0,
                      total: 15000.0,
                      slowest: 4000000.0,
                      fastest: 200000.0,
                      average: 2000000.0,
                      requestsPerSec: 5000.0,
                      totalData: 150000000,
                      sizePerRequest: 10000.0,
                      sizePerSec: 50000000.0,
                      errorDistribution: {},
                      p95_latency: 3.8,
                    },
                  },
                  {
                    server: "puma",
                    test_case: "chunked",
                    version:
                      "Puma version 6.6.0+h2o version 2.3.0-DEV@87e2aa634 (Ruby 3.2.2) with HTTP/2 support enabled",
                    threads: 1,
                    workers: 1,
                    http2: true,
                    concurrency: 50,
                    results: {
                      successRate: 1.0,
                      total: 22000.0,
                      slowest: 12000000.0,
                      fastest: 400000.0,
                      average: 6000000.0,
                      requestsPerSec: 7500.0,
                      totalData: 220000000,
                      sizePerRequest: 10000.0,
                      sizePerSec: 75000000.0,
                      errorDistribution: {},
                      p95_latency: 11.2,
                    },
                  },
                ],
              },
            ],
          },
          {
            test: "io_party",
            servers: [
              {
                server: "itsi",
                results: [
                  {
                    server: "itsi",
                    test_case: "io_party",
                    version:
                      "ITSI v0.9.1-beta.3 (Experimental) with advanced IO processing and HTTP/2 multiplexing support",
                    threads: 1,
                    workers: 1,
                    http2: true,
                    concurrency: 10,
                    rss_mb: 64.11,
                    results: {
                      successRate: 1.0,
                      total: 48591.0,
                      slowest: 2850375.0,
                      fastest: 116292.0,
                      average: 486975.0,
                      requestsPerSec: 16196.49,
                      totalData: 0,
                      sizePerRequest: 0.0,
                      sizePerSec: 0.0,
                      errorDistribution: {},
                      p95_latency: 0.977999,
                    },
                    timestamp: "2025-05-15T03:43:50Z",
                  },
                ],
              },
            ],
          },
        ],
      },
      {
        group: "sinatra",
        tests: [
          {
            test: "hello_world",
            servers: [
              {
                server: "falcon",
                results: [
                  {
                    server: "falcon",
                    test_case: "hello_world",
                    version:
                      "Falcon v0.51.1 (Ruby 3.3.0-preview1) with HTTP/2 and WebSocket support, running on Async::HTTP::Protocol::HTTP2 implementation",
                    threads: 2,
                    workers: 2,
                    http2: true,
                    concurrency: 10,
                    results: {
                      successRate: 1.0,
                      total: 30000.0,
                      slowest: 10000000.0,
                      fastest: 400000.0,
                      average: 5000000.0,
                      requestsPerSec: 10000.0,
                      totalData: 0,
                      sizePerRequest: 0.0,
                      sizePerSec: 0.0,
                      errorDistribution: {},
                      p95_latency: 8.5,
                    },
                  },
                ],
              },
            ],
          },
        ],
      },
    ],
  },
]

export default function Home() {
  const [isLoading, setIsLoading] = useState(true)
  const [benchmarkData, setBenchmarkData] = useState([])
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const fetchData = async () => {
      try {
        // In a real app, this would fetch from your API endpoint
        // For demo purposes, we're using the sample hierarchical data
        setBenchmarkData(sampleHierarchicalData)
        setIsLoading(false)
      } catch (err) {
        console.error("Error fetching benchmark data:", err)
        setError("Failed to load benchmark data. Please try again.")
        setIsLoading(false)
      }
    }

    fetchData()
  }, [])

  if (isLoading) {
    return (
      <div className="flex h-screen items-center justify-center">
        <LoadingSpinner />
        <span className="ml-2">Loading benchmark data...</span>
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex h-screen items-center justify-center">
        <div className="text-center">
          <h2 className="text-2xl font-bold text-red-600">Error</h2>
          <p className="mt-2">{error}</p>
        </div>
      </div>
    )
  }

  return (
    <main className="container mx-auto p-2">
      <BenchmarkDashboard data={benchmarkData} />
    </main>
  )
}
