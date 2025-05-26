"use client";

import type React from "react";

import { useState, useMemo, useCallback, useEffect } from "react";
import {
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  ResponsiveContainer,
  BarChart,
} from "recharts";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  SelectGroup,
  SelectLabel,
} from "@/components/ui/select";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  InfoIcon,
  TrendingUp,
  TrendingDown,
  TagIcon,
  TrophyIcon,
} from "lucide-react";

// Updated types to match the new hierarchical structure with groups
type BenchmarkResult = {
  server: string;
  version?: string;
  test_case: string;
  threads: number;
  workers: number;
  http2: boolean;
  concurrency: number;
  rss_mb?: number;
  results: {
    successRate: number;
    total: number;
    slowest: number;
    fastest: number;
    average: number;
    requestsPerSec: number;
    totalData: number;
    sizePerRequest: number;
    sizePerSec: number;
    errorDistribution: Record<string, number>;
    p95_latency: number;
  };
  timestamp?: string;
};

type ServerData = {
  server: string;
  results: BenchmarkResult[];
};

type TestData = {
  test: string;
  servers: ServerData[];
};

type GroupData = {
  group: string;
  tests: TestData[];
};

type CpuData = {
  cpu: string;
  groups: GroupData[];
};

type HierarchicalBenchmarkData = CpuData[];

type FilterOptions = {
  cpus: string[];
  testCases: string[];
  servers: string[];
  threads: number[];
  workers: number[];
  concurrencyLevels: number[];
  http2Options: (boolean | "all")[];
};

type FilterState = {
  cpu: string;
  testCase: string;
  threads: number;
  workers: number;
  concurrency: number;
  http2: boolean | "all";
  xAxis: string;
  metric: string;
  visibleServers: string[];
};

type BenchmarkDashboardProps = {
  data: HierarchicalBenchmarkData;
};

export function BenchmarkDashboard({ data }: BenchmarkDashboardProps) {
  // Helper function to format server names (replace __ with +)
  const formatServerName = useCallback((serverName: string): string => {
    return serverName.replace(/__/g, "+");
  }, []);

  // Flatten the hierarchical data into the format we need for processing
  const flattenedData = useMemo(() => {
    const flattened: BenchmarkResult[] = [];

    if (!data || !Array.isArray(data)) {
      return flattened;
    }

    data.forEach((cpuData) => {
      if (!cpuData?.groups || !Array.isArray(cpuData.groups)) {
        return;
      }

      cpuData.groups.forEach((groupData) => {
        if (!groupData?.tests || !Array.isArray(groupData.tests)) {
          return;
        }

        groupData.tests.forEach((testData) => {
          if (!testData?.servers || !Array.isArray(testData.servers)) {
            return;
          }

          testData.servers.forEach((serverData) => {
            if (!serverData?.results || !Array.isArray(serverData.results)) {
              return;
            }

            serverData.results.forEach((result) => {
              // Add CPU and group information to each result
              flattened.push({
                ...result,
                cpu: cpuData.cpu,
                group: groupData.group,
              } as BenchmarkResult & { cpu: string; group: string });
            });
          });
        });
      });
    });

    return flattened;
  }, [data]);

  // Extract all possible filter options from hierarchical data
  const allFilterOptions: FilterOptions = useMemo(() => {
    const defaultOptions: FilterOptions = {
      cpus: [],
      testCases: [],
      servers: [],
      threads: [],
      workers: [],
      concurrencyLevels: [],
      http2Options: ["all"],
    };

    if (!data || !Array.isArray(data) || data.length === 0) {
      return defaultOptions;
    }

    return {
      cpus: data.map((cpuData) => cpuData.cpu).filter(Boolean),
      testCases: [
        ...new Set(flattenedData.map((item) => item.test_case).filter(Boolean)),
      ],
      servers: [
        ...new Set(flattenedData.map((item) => item.server).filter(Boolean)),
      ],
      threads: [
        ...new Set(
          flattenedData
            .map((item) => item.threads)
            .filter((t) => typeof t === "number"),
        ),
      ].sort((a, b) => a - b),
      workers: [
        ...new Set(
          flattenedData
            .map((item) => item.workers)
            .filter((w) => typeof w === "number"),
        ),
      ].sort((a, b) => a - b),
      concurrencyLevels: [
        ...new Set(
          flattenedData
            .map((item) => item.concurrency)
            .filter((c) => typeof c === "number"),
        ),
      ].sort((a, b) => a - b),
      http2Options: [
        "all",
        ...new Set(
          flattenedData
            .map((item) => item.http2)
            .filter((h) => typeof h === "boolean"),
        ),
      ],
    };
  }, [data, flattenedData]);

  // Generate consistent colors for all servers upfront
  const serverColors = useMemo(() => {
    const itsiColor = "#ff7f0e";

    const palette = [
      "#1f77b4", // blue
      "#2ca02c", // green
      "#d62728", // red (not orange)
      "#9467bd", // purple
      "#8c564b", // brown
      "#e377c2", // pink
      "#7f7f7f", // gray
      "#bcbd22", // lime
      "#17becf", // cyan
      "#393b79", // indigo
      "#a55194", // magenta
    ];

    const colorMap: Record<string, string> = {};
    const servers = [...allFilterOptions.servers];

    const itsiIndex = servers.findIndex((s) => s === "itsi");
    if (itsiIndex !== -1) {
      colorMap["itsi"] = itsiColor;
      servers.splice(itsiIndex, 1);
    }

    servers.forEach((server, index) => {
      colorMap[server] = palette[index % palette.length];
    });

    return colorMap;
  }, [allFilterOptions.servers]);

  // Helper function to parse URL search parameters
  const parseUrlParams = useCallback((): Partial<FilterState> | null => {
    if (typeof window === "undefined") return null;

    try {
      const urlParams = new URLSearchParams(window.location.search);
      const filters: Partial<FilterState> = {};

      // Parse each parameter
      const cpu = urlParams.get("cpu");
      const testCase = urlParams.get("testCase");
      const threads = urlParams.get("threads");
      const workers = urlParams.get("workers");
      const concurrency = urlParams.get("concurrency");
      const http2 = urlParams.get("http2");
      const xAxis = urlParams.get("xAxis");
      const metric = urlParams.get("metric");

      if (cpu) filters.cpu = cpu;
      if (testCase) filters.testCase = testCase;
      if (threads) filters.threads = Number.parseInt(threads);
      if (workers) filters.workers = Number.parseInt(workers);
      if (concurrency) filters.concurrency = Number.parseInt(concurrency);
      if (http2) {
        if (http2 === "all") {
          filters.http2 = "all";
        } else {
          filters.http2 = http2 === "true";
        }
      }
      if (xAxis) filters.xAxis = xAxis;
      if (metric) filters.metric = metric;

      const visibleServersParam = urlParams.get("visibleServers");
      if (visibleServersParam) {
        try {
          filters.visibleServers = visibleServersParam
            .split(",")
            .filter(Boolean);
        } catch (e) {
          // Ignore parsing errors
        }
      }

      return Object.keys(filters).length > 0 ? filters : null;
    } catch (error) {
      console.warn("Failed to parse URL parameters:", error);
    }

    return null;
  }, []);

  // Helper function to update URL search parameters
  const updateUrlParams = useCallback((filters: FilterState) => {
    if (typeof window === "undefined") return;

    try {
      const url = new URL(window.location.href);

      // Set each parameter
      url.searchParams.set("cpu", filters.cpu);
      url.searchParams.set("testCase", filters.testCase);
      url.searchParams.set("threads", filters.threads.toString());
      url.searchParams.set("workers", filters.workers.toString());
      url.searchParams.set("concurrency", filters.concurrency.toString());
      url.searchParams.set("http2", filters.http2.toString());
      url.searchParams.set("xAxis", filters.xAxis);
      url.searchParams.set("metric", filters.metric);

      // Set visible servers as comma-separated list
      if (filters.visibleServers && filters.visibleServers.length > 0) {
        url.searchParams.set(
          "visibleServers",
          filters.visibleServers.join(","),
        );
      } else {
        url.searchParams.delete("visibleServers");
      }

      // Update the URL without triggering a page reload
      window.history.replaceState(null, "", url.toString());
    } catch (error) {
      console.warn("Failed to update URL parameters:", error);
    }
  }, []);

  // Helper function to validate and sanitize filter state from URL
  const validateAndSanitizeFilters = useCallback(
    (urlFilters: Partial<FilterState>): FilterState => {
      const defaultFilters: FilterState = {
        cpu: allFilterOptions.cpus[0] || "",
        testCase: allFilterOptions.testCases[0] || "",
        threads: allFilterOptions.threads[0] || 1,
        workers: allFilterOptions.workers[0] || 1,
        concurrency: allFilterOptions.concurrencyLevels[0] || 10,
        http2: "all", // Default to "all"
        xAxis: "concurrency",
        metric: "rps",
        visibleServers: allFilterOptions.servers, // Default to all servers visible
      };

      // Validate each field and fall back to defaults if invalid
      const validatedFilters: FilterState = {
        cpu: allFilterOptions.cpus.includes(urlFilters.cpu || "")
          ? urlFilters.cpu!
          : defaultFilters.cpu,
        testCase: allFilterOptions.testCases.includes(urlFilters.testCase || "")
          ? urlFilters.testCase!
          : defaultFilters.testCase,
        threads: allFilterOptions.threads.includes(urlFilters.threads || 0)
          ? urlFilters.threads!
          : defaultFilters.threads,
        workers: allFilterOptions.workers.includes(urlFilters.workers || 0)
          ? urlFilters.workers!
          : defaultFilters.workers,
        concurrency: allFilterOptions.concurrencyLevels.includes(
          urlFilters.concurrency || 0,
        )
          ? urlFilters.concurrency!
          : defaultFilters.concurrency,
        http2: allFilterOptions.http2Options.includes(urlFilters.http2 as any)
          ? (urlFilters.http2 as boolean | "all")
          : defaultFilters.http2,
        xAxis: ["concurrency", "threads", "workers"].includes(
          urlFilters.xAxis || "",
        )
          ? urlFilters.xAxis!
          : defaultFilters.xAxis,
        metric: ["rps", "p95_latency", "errorRate"].includes(
          urlFilters.metric || "",
        )
          ? urlFilters.metric!
          : defaultFilters.metric,
        visibleServers: Array.isArray(urlFilters.visibleServers)
          ? urlFilters.visibleServers.filter((server) =>
              allFilterOptions.servers.includes(server),
            )
          : defaultFilters.visibleServers,
      };

      return validatedFilters;
    },
    [allFilterOptions],
  );

  // Initialize filter state with URL parameters or defaults
  const [filters, setFilters] = useState<FilterState>(() => {
    const urlFilters = parseUrlParams();
    if (urlFilters && allFilterOptions.cpus.length > 0) {
      return validateAndSanitizeFilters(urlFilters);
    }

    const preferredTestCase = allFilterOptions.testCases.includes("hello_world")
      ? "hello_world"
      : allFilterOptions.testCases[0] || "";

    return {
      cpu: allFilterOptions.cpus[0] || "",
      testCase: preferredTestCase || "",
      threads: allFilterOptions.threads[0] || 1,
      workers: allFilterOptions.workers[0] || 1,
      concurrency: allFilterOptions.concurrencyLevels[0] || 10,
      http2: "all", // Default to "all"
      xAxis: "concurrency",
      metric: "rps",
      visibleServers: allFilterOptions.servers,
    };
  });

  // Track which servers are visible
  const [visibleServers, setVisibleServers] = useState<Record<string, boolean>>(
    () => {
      const initialVisibleServers: Record<string, boolean> = {};
      allFilterOptions.servers.forEach((server) => {
        initialVisibleServers[server] = true;
      });

      const urlFilters = parseUrlParams();
      if (
        urlFilters?.visibleServers &&
        Array.isArray(urlFilters.visibleServers)
      ) {
        allFilterOptions.servers.forEach((server) => {
          initialVisibleServers[server] =
            urlFilters.visibleServers!.includes(server);
        });
      }

      return initialVisibleServers;
    },
  );

  // Currently hovered data point
  const [hoveredPoint, setHoveredPoint] = useState<BenchmarkResult | null>(
    null,
  );
  const [activeDataKey, setActiveDataKey] = useState<string | null>(null);

  // Track legend interactions to disable animations
  // const [isLegendInteracting, setIsLegendInteracting] = useState(false)

  // Update URL parameters when filters change
  useEffect(() => {
    updateUrlParams(filters);
  }, [filters, updateUrlParams]);

  // Handle initial load from URL parameters after data is available
  useEffect(() => {
    if (allFilterOptions.cpus.length > 0) {
      const urlFilters = parseUrlParams();
      if (urlFilters) {
        const validatedFilters = validateAndSanitizeFilters(urlFilters);
        setFilters(validatedFilters);

        // Update visibleServers based on URL params after filters are set
        const initialVisibleServers: Record<string, boolean> = {};
        allFilterOptions.servers.forEach((server) => {
          initialVisibleServers[server] = true;
        });

        if (
          urlFilters?.visibleServers &&
          Array.isArray(urlFilters.visibleServers)
        ) {
          allFilterOptions.servers.forEach((server) => {
            initialVisibleServers[server] =
              urlFilters.visibleServers!.includes(server);
          });
        }
        setVisibleServers(initialVisibleServers);
      }
    }
  }, [allFilterOptions, parseUrlParams, validateAndSanitizeFilters]);

  // Get dynamic filter options based on selected CPU and test case
  const filterOptions = useMemo(() => {
    // First filter by CPU
    const cpuFilteredData = flattenedData.filter(
      (item) => (item as any).cpu === filters.cpu,
    );

    // Get available test cases for the selected CPU
    const availableTestCases = [
      ...new Set(cpuFilteredData.map((item) => item.test_case).filter(Boolean)),
    ];

    // Then filter by test case to get the remaining filter options
    const testCaseFilteredData = cpuFilteredData.filter(
      (item) => item.test_case === filters.testCase,
    );

    return {
      cpus: allFilterOptions.cpus,
      testCases: availableTestCases,
      servers: [
        ...new Set(
          testCaseFilteredData.map((item) => item.server).filter(Boolean),
        ),
      ],
      threads: [
        ...new Set(
          testCaseFilteredData
            .map((item) => item.threads)
            .filter((t) => typeof t === "number"),
        ),
      ].sort((a, b) => a - b),
      workers: [
        ...new Set(
          testCaseFilteredData
            .map((item) => item.workers)
            .filter((w) => typeof w === "number"),
        ),
      ].sort((a, b) => a - b),
      concurrencyLevels: [
        ...new Set(
          testCaseFilteredData
            .map((item) => item.concurrency)
            .filter((c) => typeof c === "number"),
        ),
      ].sort((a, b) => a - b),
      http2Options: [
        "all",
        ...new Set(
          testCaseFilteredData
            .map((item) => item.http2)
            .filter((h) => typeof h === "boolean"),
        ),
      ],
    };
  }, [flattenedData, filters.cpu, filters.testCase, allFilterOptions.cpus]);

  // Get grouped test cases for the dropdown
  const groupedTestCases = useMemo(() => {
    // First filter by CPU to get available data
    const cpuFilteredData = flattenedData.filter(
      (item) => (item as any).cpu === filters.cpu,
    );

    // Group test cases by their group
    const groupedTests: Record<string, string[]> = {};
    cpuFilteredData.forEach((item) => {
      const group = (item as any).group;
      if (!group || !item.test_case) return;

      if (!groupedTests[group]) {
        groupedTests[group] = [];
      }
      if (!groupedTests[group].includes(item.test_case)) {
        groupedTests[group].push(item.test_case);
      }
    });

    // Sort test cases within each group
    Object.keys(groupedTests).forEach((group) => {
      groupedTests[group].sort();
    });

    // Sort groups: "rack" first, then alphabetically
    const sortedGroups = Object.keys(groupedTests).sort((a, b) => {
      if (a === "rack") return -1;
      if (b === "rack") return 1;
      return a.localeCompare(b);
    });

    return { groupedTests, sortedGroups };
  }, [flattenedData, filters.cpu]);

  // Update filters when CPU or test case changes to ensure valid selections
  useEffect(() => {
    setFilters((prev) => {
      const newFilters = { ...prev };

      // If test case is not available for selected CPU, select first available
      if (!filterOptions.testCases.includes(prev.testCase)) {
        newFilters.testCase = filterOptions.testCases[0] || "";
      }

      // Update other filters with first available value if current value is not valid
      // BUT skip the filter that matches the current X-axis
      if (
        prev.xAxis !== "threads" &&
        !filterOptions.threads.includes(prev.threads)
      ) {
        newFilters.threads = filterOptions.threads[0] || 1;
      }

      if (
        prev.xAxis !== "workers" &&
        !filterOptions.workers.includes(prev.workers)
      ) {
        newFilters.workers = filterOptions.workers[0] || 1;
      }

      if (
        prev.xAxis !== "concurrency" &&
        !filterOptions.concurrencyLevels.includes(prev.concurrency)
      ) {
        newFilters.concurrency = filterOptions.concurrencyLevels[0] || 10;
      }

      if (!filterOptions.http2Options.includes(prev.http2)) {
        newFilters.http2 = filterOptions.http2Options[0] || "all";
      }

      return newFilters;
    });
  }, [filters.cpu, filterOptions]);

  // Apply filters to flattened data
  const filteredData = useMemo(() => {
    return flattenedData.filter((item) => {
      const itemWithCpu = item as any;
      if (itemWithCpu.cpu !== filters.cpu) return false;
      if (item.test_case !== filters.testCase) return false;

      // Don't filter by the parameter that's being used as X-axis
      if (filters.xAxis !== "threads" && item.threads !== filters.threads)
        return false;
      if (filters.xAxis !== "workers" && item.workers !== filters.workers)
        return false;
      if (
        filters.xAxis !== "concurrency" &&
        item.concurrency !== filters.concurrency
      )
        return false;

      // Handle "all" protocol option
      if (filters.http2 !== "all" && item.http2 !== filters.http2) return false;

      return true;
    });
  }, [flattenedData, filters]);

  // Prepare data for chart based on selected x-axis
  const chartData = useMemo(() => {
    // Group data by the selected x-axis
    const groupedByXAxis: Record<string, BenchmarkResult[]> = {};

    filteredData.forEach((item) => {
      const xAxisValue = String(item[filters.xAxis as keyof BenchmarkResult]);
      if (!groupedByXAxis[xAxisValue]) {
        groupedByXAxis[xAxisValue] = [];
      }
      groupedByXAxis[xAxisValue].push(item);
    });

    // Convert to format suitable for chart
    return Object.entries(groupedByXAxis)
      .map(([xAxisValue, items]) => {
        const point: Record<string, any> = { [filters.xAxis]: xAxisValue };

        // Group items by server and protocol when "all" is selected
        items.forEach((item) => {
          if (visibleServers[item.server]) {
            // Single metric based on selection
            let metricValue: number;
            switch (filters.metric) {
              case "rps":
                metricValue = item.results.requestsPerSec;
                break;
              case "p95_latency":
                metricValue = item.results.p95_latency;
                break;
              case "errorRate":
                metricValue = 1 - item.results.successRate;
                break;
              default:
                metricValue = item.results.requestsPerSec;
            }

            // Create unique key for server+protocol combination when showing all protocols
            let dataKey: string;
            if (filters.http2 === "all") {
              const protocolSuffix = item.http2 ? " (HTTP/2)" : " (HTTP/1.1)";
              dataKey = `${formatServerName(item.server)}${protocolSuffix}`;
            } else {
              dataKey = formatServerName(item.server);
            }

            point[dataKey] = metricValue;
            // Store the full item for hover details
            point[`${dataKey}_data`] = item;
          }
        });

        return point;
      })
      .sort((a, b) => {
        // Sort numerically if the x-axis is a number
        const aVal = a[filters.xAxis];
        const bVal = b[filters.xAxis];
        if (!isNaN(Number(aVal)) && !isNaN(Number(bVal))) {
          return Number(aVal) - Number(bVal);
        }
        // Otherwise sort alphabetically
        return String(aVal).localeCompare(String(bVal));
      });
  }, [filteredData, filters, visibleServers, formatServerName]);

  // Get metric info for display
  const metricInfo = useMemo(() => {
    // Helper function to format large numbers compactly
    const formatCompact = (value: number, decimals = 1): string => {
      if (value >= 1000000) {
        return `${(value / 1000000).toFixed(decimals)}M`;
      } else if (value >= 1000) {
        return `${(value / 1000).toFixed(decimals)}K`;
      }
      return value.toFixed(decimals);
    };

    switch (filters.metric) {
      case "rps":
        return {
          label: "Requests per Second",
          isBetter: "higher",
          icon: TrendingUp,
          formatter: (value: number) => formatCompact(value, 1),
        };
      case "p95_latency":
        return {
          label: "P95 Latency (ms)",
          isBetter: "lower",
          icon: TrendingDown,
          formatter: (value: number) =>
            value >= 1000 ? formatCompact(value, 1) : value.toFixed(2),
        };
      case "errorRate":
        return {
          label: "Error Rate",
          isBetter: "lower",
          icon: TrendingDown,
          formatter: (value: number) => `${(value * 100).toFixed(1)}%`,
        };
      default:
        return {
          label: "Requests per Second",
          isBetter: "higher",
          icon: TrendingUp,
          formatter: (value: number) => formatCompact(value, 1),
        };
    }
  }, [filters.metric]);

  // Handle filter changes
  const handleFilterChange = (key: keyof FilterState, value: any) => {
    setFilters((prev) => ({ ...prev, [key]: value }));
  };

  // Toggle server visibility when clicking on legend
  const handleLegendClick = useCallback(
    (server: string, event?: React.MouseEvent) => {
      // Disable animations during legend interaction
      // setIsLegendInteracting(true)

      // Check if Ctrl (Windows/Linux) or Cmd (Mac) key is pressed
      const isExclusiveMode = event?.ctrlKey || event?.metaKey;

      if (isExclusiveMode) {
        // Ctrl/Cmd + click: Show only this server (hide all others)
        const newVisibleServers: Record<string, boolean> = {};
        allFilterOptions.servers.forEach((s) => {
          newVisibleServers[s] = s === server;
        });
        setVisibleServers(newVisibleServers);
      } else {
        // Normal click: Toggle this server
        setVisibleServers((prev) => ({
          ...prev,
          [server]: !prev[server],
        }));
      }

      // Re-enable animations after a short delay
      // setTimeout(() => {
      //   setIsLegendInteracting(false)
      // }, 100)
    },
    [allFilterOptions.servers],
  );

  // Calculate summary statistics
  const summaryStats = useMemo(() => {
    if (filteredData.length === 0) return null;

    const values = filteredData.map((item) => {
      switch (filters.metric) {
        case "rps":
          return item.results.requestsPerSec;
        case "p95_latency":
          return item.results.p95_latency;
        case "errorRate":
          return 1 - item.results.successRate;
        default:
          return item.results.requestsPerSec;
      }
    });

    return {
      count: filteredData.length,
      min: Math.min(...values),
      max: Math.max(...values),
      avg: values.reduce((sum, val) => sum + val, 0) / values.length,
    };
  }, [filteredData, filters.metric]);

  const topPerformers = useMemo(() => {
    if (filteredData.length === 0) return [];

    const currentGroup = (filteredData[0] as any).group;
    if (!currentGroup) return [];

    const performanceMap: Record<
      string,
      Record<string, Record<boolean, number[]>>
    > = {};

    flattenedData
      .filter((item) => {
        const itemWithCpu = item as any;
        return (
          itemWithCpu.cpu === filters.cpu && itemWithCpu.group === currentGroup
        );
      })
      .forEach((item) => {
        const key = `${item.test_case}_${item.threads}_${item.workers}_${item.concurrency}`;

        if (!performanceMap[key]) {
          performanceMap[key] = {};
        }

        if (!performanceMap[key][item.server]) {
          performanceMap[key][item.server] = { true: [], false: [] };
        }

        performanceMap[key][item.server][item.http2].push(
          item.results.requestsPerSec,
        );
      });

    const winCounts: Record<string, number> = {};

    Object.values(performanceMap).forEach((serverResults) => {
      const allScores: [string, number][] = [];

      for (const [server, variants] of Object.entries(serverResults)) {
        for (const [http2, values] of Object.entries(variants)) {
          const parsedHttp2 = http2 === "true"; // keys are string
          if (values.length > 0) {
            const avg = values.reduce((sum, v) => sum + v, 0) / values.length;
            allScores.push([`${server}___${parsedHttp2}`, avg]);
          }
        }
      }

      if (allScores.length === 0) return;

      const bestScore = Math.max(...allScores.map(([, v]) => v));
      const winningServers = new Set(
        allScores
          .filter(([, v]) => v === bestScore)
          .map(([k]) => k.split("___")[0]), // extract server
      );

      for (const server of winningServers) {
        winCounts[server] = (winCounts[server] || 0) + 1;
      }
    });

    return Object.entries(winCounts)
      .sort(([, a], [, b]) => b - a)
      .slice(0, 8)
      .map(([server, count]) => ({ server, count }));
  }, [flattenedData, filters.cpu, filteredData]);

  // Handle chart hover
  const handleChartHover = (props: any) => {
    if (props.activePayload && props.activePayload.length > 0) {
      const { dataKey, payload } = props.activePayload[0];
      const serverData = payload[`${dataKey}_data`];

      if (serverData) {
        setHoveredPoint(serverData);
        setActiveDataKey(dataKey);
        return;
      }
    }

    setHoveredPoint(null);
    setActiveDataKey(null);
  };

  // Get visible servers and their data keys for the chart
  const visibleDataKeys = useMemo(() => {
    const keys: string[] = [];

    filterOptions.servers.forEach((server) => {
      if (visibleServers[server]) {
        if (filters.http2 === "all") {
          // Check if this server has both HTTP/1.1 and HTTP/2 data
          const serverData = filteredData.filter(
            (item) => item.server === server,
          );
          const hasHttp1 = serverData.some((item) => !item.http2);
          const hasHttp2 = serverData.some((item) => item.http2);

          if (hasHttp1) keys.push(`${formatServerName(server)} (HTTP/1.1)`);
          if (hasHttp2) keys.push(`${formatServerName(server)} (HTTP/2)`);
        } else {
          keys.push(formatServerName(server));
        }
      }
    });

    return keys;
  }, [
    filterOptions.servers,
    visibleServers,
    filters.http2,
    filteredData,
    formatServerName,
  ]);

  // Get protocol display text
  const getProtocolDisplay = () => {
    if (filters.http2 === "all") return "All Protocols";
    return filters.http2 ? "HTTP/2" : "HTTP/1.1";
  };

  // Show loading state if no data
  const visibleServersForUrl = useMemo(() => {
    return Object.keys(visibleServers).filter(
      (server) => visibleServers[server],
    );
  }, [visibleServers]);

  // Update URL parameters when visible servers change
  useEffect(() => {
    updateUrlParams({ ...filters, visibleServers: visibleServersForUrl });
  }, [visibleServersForUrl, filters, updateUrlParams]);

  if (!data || !Array.isArray(data) || data.length === 0) {
    return (
      <div className="flex h-64 items-center justify-center">
        <p className="text-gray-500">No benchmark data available</p>
      </div>
    );
  }

  return (
    <div className="grid grid-cols-1 gap-2">
      <div className="grid grid-cols-1 md:grid-cols-12 gap-2">
        {/* Left column: Filters */}
        <div className="md:col-span-3">
          <Card className="shadow-sm h-full">
            <CardHeader className="py-2 px-3">
              <CardTitle className="text-sm">Filters</CardTitle>
            </CardHeader>
            <CardContent className="p-2">
              <div className="space-y-2">
                {/* CPU filter */}
                <div className="space-y-1">
                  <Label htmlFor="cpu-filter" className="text-xs">
                    CPU
                  </Label>
                  <Select
                    value={filters.cpu}
                    onValueChange={(value) => handleFilterChange("cpu", value)}
                  >
                    <SelectTrigger id="cpu-filter" className="h-7 text-xs">
                      <SelectValue placeholder="Select CPU" />
                    </SelectTrigger>
                    <SelectContent>
                      {filterOptions.cpus.map((cpu) => (
                        <SelectItem key={cpu} value={cpu} className="text-xs">
                          {cpu}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>

                {/* Test case filter with groups */}
                <div className="space-y-1">
                  <Label htmlFor="test-case-filter" className="text-xs">
                    Test Case
                  </Label>
                  <Select
                    value={filters.testCase}
                    onValueChange={(value) =>
                      handleFilterChange("testCase", value)
                    }
                  >
                    <SelectTrigger
                      id="test-case-filter"
                      className="h-7 text-xs"
                    >
                      <SelectValue placeholder="Select test" />
                    </SelectTrigger>
                    <SelectContent>
                      {groupedTestCases.sortedGroups.map((group) => (
                        <SelectGroup key={group}>
                          <SelectLabel className="text-xs font-medium text-muted-foreground pl-2">
                            {group}
                          </SelectLabel>
                          {groupedTestCases.groupedTests[group].map(
                            (testCase) => (
                              <SelectItem
                                key={testCase}
                                value={testCase}
                                className="text-xs pl-6"
                              >
                                {testCase}
                              </SelectItem>
                            ),
                          )}
                        </SelectGroup>
                      ))}
                    </SelectContent>
                  </Select>
                </div>

                <div className="grid grid-cols-2 gap-2">
                  {/* Threads filter - disabled if xAxis is threads */}
                  <div className="space-y-1">
                    <Label htmlFor="threads-filter" className="text-xs">
                      Threads
                    </Label>
                    <Select
                      value={filters.threads.toString()}
                      onValueChange={(value) =>
                        handleFilterChange("threads", Number.parseInt(value))
                      }
                      disabled={filters.xAxis === "threads"}
                    >
                      <SelectTrigger
                        id="threads-filter"
                        className="h-7 text-xs"
                      >
                        <SelectValue
                          placeholder={
                            filters.xAxis === "threads"
                              ? "On X-Axis"
                              : "Select threads"
                          }
                        />
                      </SelectTrigger>
                      <SelectContent>
                        {filterOptions.threads.length > 0 ? (
                          filterOptions.threads.map((thread) => (
                            <SelectItem
                              key={thread}
                              value={thread.toString()}
                              className="text-xs"
                            >
                              {thread}
                            </SelectItem>
                          ))
                        ) : (
                          <SelectItem value="1" className="text-xs">
                            1
                          </SelectItem>
                        )}
                      </SelectContent>
                    </Select>
                  </div>

                  {/* Workers filter - disabled if xAxis is workers */}
                  <div className="space-y-1">
                    <Label htmlFor="workers-filter" className="text-xs">
                      Workers
                    </Label>
                    <Select
                      value={filters.workers.toString()}
                      onValueChange={(value) =>
                        handleFilterChange("workers", Number.parseInt(value))
                      }
                      disabled={filters.xAxis === "workers"}
                    >
                      <SelectTrigger
                        id="workers-filter"
                        className="h-7 text-xs"
                      >
                        <SelectValue
                          placeholder={
                            filters.xAxis === "workers"
                              ? "On X-Axis"
                              : "Select workers"
                          }
                        />
                      </SelectTrigger>
                      <SelectContent>
                        {filterOptions.workers.length > 0 ? (
                          filterOptions.workers.map((worker) => (
                            <SelectItem
                              key={worker}
                              value={worker.toString()}
                              className="text-xs"
                            >
                              {worker}
                            </SelectItem>
                          ))
                        ) : (
                          <SelectItem value="1" className="text-xs">
                            1
                          </SelectItem>
                        )}
                      </SelectContent>
                    </Select>
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-2">
                  {/* Concurrency filter - disabled if xAxis is concurrency */}
                  <div className="space-y-1">
                    <Label htmlFor="concurrency-filter" className="text-xs">
                      Concurrency
                    </Label>
                    <Select
                      value={filters.concurrency.toString()}
                      onValueChange={(value) =>
                        handleFilterChange(
                          "concurrency",
                          Number.parseInt(value),
                        )
                      }
                      disabled={filters.xAxis === "concurrency"}
                    >
                      <SelectTrigger
                        id="concurrency-filter"
                        className="h-7 text-xs"
                      >
                        <SelectValue
                          placeholder={
                            filters.xAxis === "concurrency"
                              ? "On X-Axis"
                              : "Select concurrency"
                          }
                        />
                      </SelectTrigger>
                      <SelectContent>
                        {filterOptions.concurrencyLevels.length > 0 ? (
                          filterOptions.concurrencyLevels.map((level) => (
                            <SelectItem
                              key={level}
                              value={level.toString()}
                              className="text-xs"
                            >
                              {level}
                            </SelectItem>
                          ))
                        ) : (
                          <SelectItem value="10" className="text-xs">
                            10
                          </SelectItem>
                        )}
                      </SelectContent>
                    </Select>
                  </div>

                  {/* HTTP2 filter */}
                  <div className="space-y-1">
                    <Label htmlFor="http2-filter" className="text-xs">
                      Protocol
                    </Label>
                    <Select
                      value={filters.http2.toString()}
                      onValueChange={(value) => {
                        if (value === "all") {
                          handleFilterChange("http2", "all");
                        } else {
                          handleFilterChange("http2", value === "true");
                        }
                      }}
                    >
                      <SelectTrigger id="http2-filter" className="h-7 text-xs">
                        <SelectValue placeholder="Select protocol" />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="all" className="text-xs">
                          All
                        </SelectItem>
                        {filterOptions.http2Options
                          .filter((option) => option !== "all")
                          .map((option) => (
                            <SelectItem
                              key={String(option)}
                              value={String(option)}
                              className="text-xs"
                            >
                              {option ? "HTTP/2" : "HTTP/1.1"}
                            </SelectItem>
                          ))}
                      </SelectContent>
                    </Select>
                  </div>
                </div>

                {/* X-Axis selection */}
                <div className="space-y-1">
                  <Label htmlFor="x-axis-select" className="text-xs">
                    X-Axis
                  </Label>
                  <Select
                    value={filters.xAxis}
                    onValueChange={(value) =>
                      handleFilterChange("xAxis", value)
                    }
                  >
                    <SelectTrigger id="x-axis-select" className="h-7 text-xs">
                      <SelectValue placeholder="X-Axis" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="concurrency" className="text-xs">
                        Concurrency
                      </SelectItem>
                      <SelectItem value="threads" className="text-xs">
                        Threads
                      </SelectItem>
                      <SelectItem value="workers" className="text-xs">
                        Workers
                      </SelectItem>
                    </SelectContent>
                  </Select>
                </div>

                {/* Metric selection */}
                <div className="space-y-1">
                  <Label htmlFor="metric-select" className="text-xs">
                    Metric
                  </Label>
                  <Tabs
                    value={filters.metric}
                    onValueChange={(value) =>
                      handleFilterChange("metric", value)
                    }
                    className="w-full"
                  >
                    <TabsList className="h-6 w-full">
                      <TabsTrigger
                        value="rps"
                        className="text-xs px-2 py-0 h-4 flex-1"
                      >
                        RPS
                      </TabsTrigger>
                      <TabsTrigger
                        value="p95_latency"
                        className="text-xs px-2 py-0 h-4 flex-1"
                      >
                        P95
                      </TabsTrigger>
                      <TabsTrigger
                        value="errorRate"
                        className="text-xs px-2 py-0 h-4 flex-1"
                      >
                        Errors
                      </TabsTrigger>
                    </TabsList>
                  </Tabs>

                  {/* Better indicator */}
                  <div className="flex items-center justify-center text-xs text-muted-foreground">
                    <metricInfo.icon className="h-3 w-3 mr-1" />
                    <span>{metricInfo.isBetter} is better</span>
                  </div>
                </div>

                {/* Compact Summary Stats */}
                {summaryStats && (
                  <div className="pt-1 space-y-1 text-xs">
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">Results:</span>
                      <span className="font-medium">{summaryStats.count}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">Min:</span>
                      <span className="font-medium">
                        {metricInfo.formatter(summaryStats.min)}
                      </span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">Max:</span>
                      <span className="font-medium">
                        {metricInfo.formatter(summaryStats.max)}
                      </span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">Avg:</span>
                      <span className="font-medium">
                        {metricInfo.formatter(summaryStats.avg)}
                      </span>
                    </div>
                  </div>
                )}
              </div>
            </CardContent>
          </Card>
        </div>

        {/* Right column: Chart and Error Distribution */}
        <div className="md:col-span-9">
          <div className="space-y-2">
            {/* Main Chart */}
            <Card className="shadow">
              <CardHeader className="pb-0 pt-2 px-3">
                <CardTitle className="text-base">
                  {metricInfo.label}: {filters.cpu} - {filters.testCase}
                  <span className="text-xs font-normal ml-2 text-muted-foreground">
                    {filters.xAxis !== "threads" &&
                      `Threads: ${filters.threads}, `}
                    {filters.xAxis !== "workers" &&
                      `Workers: ${filters.workers}, `}
                    {filters.xAxis !== "concurrency" &&
                      `Concurrency: ${filters.concurrency}, `}
                    Protocol: {getProtocolDisplay()}
                  </span>
                </CardTitle>
              </CardHeader>
              <CardContent className="p-2">
                {chartData.length === 0 ? (
                  <div className="flex h-64 items-center justify-center">
                    <p className="text-gray-500">
                      No data available for the selected filters
                    </p>
                  </div>
                ) : (
                  <>
                    <div className="h-[280px]">
                      <ResponsiveContainer width="100%" height="100%">
                        <BarChart
                          data={chartData}
                          margin={{ top: 10, right: 30, left: 30, bottom: 20 }}
                          barCategoryGap={0}
                          barGap={0}
                        >
                          <CartesianGrid strokeDasharray="3 3" opacity={0.7} />
                          <XAxis
                            dataKey={filters.xAxis}
                            label={{
                              value:
                                filters.xAxis.charAt(0).toUpperCase() +
                                filters.xAxis.slice(1),
                              position: "insideBottom",
                              offset: -5,
                            }}
                          />

                          <YAxis
                            label={{
                              value: metricInfo.label,
                              angle: -90,
                              position: "insideLeft",
                              offset: -15,
                              style: { textAnchor: "middle" },
                            }}
                            tickFormatter={metricInfo.formatter}
                          />

                          {/* Render bars for each visible data key */}
                          {visibleDataKeys.map((dataKey, index) => (
                            <Bar
                              key={dataKey}
                              dataKey={dataKey}
                              name={dataKey}
                              fill={
                                serverColors[
                                  dataKey.split(" ")[0].replace(/\+/g, "__")
                                ] ||
                                serverColors[
                                  Object.keys(serverColors)[
                                    index % Object.keys(serverColors).length
                                  ]
                                ]
                              }
                              opacity={activeDataKey === dataKey ? 1 : 0.8}
                              stackId={index}
                              isAnimationActive={true}
                              onMouseOver={(data) =>
                                handleChartHover({
                                  activePayload: [
                                    { dataKey, payload: data.payload },
                                  ],
                                })
                              }
                              onMouseLeave={() => {
                                setHoveredPoint(null);
                                setActiveDataKey(null);
                              }}
                            />
                          ))}

                          {/* Add invisible bars for zero values to enable hovering */}
                          {visibleDataKeys.map((dataKey, index) => (
                            <Bar
                              key={`${dataKey}_hover`}
                              dataKey={dataKey}
                              name={`${dataKey}_hover`}
                              fill="transparent"
                              stroke="transparent"
                              strokeWidth={0}
                              minPointSize={20}
                              stackId={`hover_${index}`}
                              isAnimationActive={true}
                              onMouseOver={(data) => {
                                if (data.payload[dataKey] === 0) {
                                  handleChartHover({
                                    activePayload: [
                                      { dataKey, payload: data.payload },
                                    ],
                                  });
                                }
                              }}
                              onMouseLeave={() => {
                                setHoveredPoint(null);
                                setActiveDataKey(null);
                              }}
                            />
                          ))}
                        </BarChart>
                      </ResponsiveContainer>
                    </div>

                    {/* Server Toggle Legend */}
                    <div className="flex flex-wrap gap-3 justify-center mt-1">
                      {filterOptions.servers.map((server) => (
                        <div
                          key={server}
                          className={`flex items-center gap-1 px-2 py-0.5 rounded cursor-pointer transition-opacity ${
                            visibleServers[server]
                              ? "opacity-100"
                              : "opacity-40"
                          }`}
                          onClick={(event) => handleLegendClick(server, event)}
                          title={`Click to toggle, ${navigator.platform.includes("Mac") ? "Cmd" : "Ctrl"}+click to show only this server`}
                        >
                          <div
                            className="w-2 h-2 rounded-sm"
                            style={{ backgroundColor: serverColors[server] }}
                          />
                          <span className="text-xs">
                            {formatServerName(server)}
                          </span>
                        </div>
                      ))}
                    </div>
                  </>
                )}
              </CardContent>
            </Card>

            {/* Top Performers and Benchmark Details in horizontal layout */}
            <div className="grid grid-cols-1 lg:grid-cols-12 gap-2">
              <div
                className={
                  topPerformers.length > 0 ? "lg:col-span-7" : "lg:col-span-12"
                }
              >
                <Card className="shadow-sm h-full">
                  <CardHeader className="py-1 px-3">
                    <CardTitle className="text-sm flex items-center">
                      {hoveredPoint ? "Benchmark Details" : "Hover Details"}
                      {!hoveredPoint && (
                        <span className="ml-2 text-xs font-normal text-muted-foreground flex items-center">
                          <InfoIcon className="h-3 w-3 mr-1" />
                          Hover over chart bars to see details
                        </span>
                      )}
                    </CardTitle>
                  </CardHeader>
                  <CardContent className="p-2">
                    {hoveredPoint ? (
                      <div className="space-y-2">
                        {/* Header with server and test case */}
                        <div className="flex flex-wrap items-center gap-2">
                          <Badge
                            variant="outline"
                            className="flex items-center"
                          >
                            <div
                              className="w-2 h-2 mr-1 rounded-full"
                              style={{
                                backgroundColor:
                                  serverColors[hoveredPoint.server],
                              }}
                            />
                            {formatServerName(hoveredPoint.server)}
                          </Badge>
                          <Badge variant="outline">
                            {hoveredPoint.test_case}
                          </Badge>
                          <Badge variant="outline">
                            {filters.xAxis}:{" "}
                            {String(
                              hoveredPoint[
                                filters.xAxis as keyof BenchmarkResult
                              ],
                            )}
                          </Badge>
                          <Badge variant="outline">
                            {hoveredPoint.http2 ? "HTTP/2" : "HTTP/1.1"}
                          </Badge>
                        </div>

                        {/* Version information in a separate row */}
                        {hoveredPoint.version && (
                          <div className="flex items-center text-xs text-muted-foreground">
                            <TagIcon className="h-3 w-3 mr-1" />
                            <span className="font-medium overflow-hidden text-ellipsis">
                              {hoveredPoint.version}
                            </span>
                          </div>
                        )}

                        {/* Performance metrics */}
                        <div className="grid grid-cols-2 md:grid-cols-2 gap-x-2 gap-y-1 text-xs">
                          <div className="flex justify-between">
                            <span className="text-muted-foreground">RPS:</span>
                            <span className="font-medium">
                              {hoveredPoint.results.requestsPerSec.toFixed(2)}
                            </span>
                          </div>
                          <div className="flex justify-between">
                            <span className="text-muted-foreground">
                              Success Rate:
                            </span>
                            <span
                              className={`font-medium ${hoveredPoint.results.successRate < 1 ? "text-red-600" : ""}`}
                            >
                              {(hoveredPoint.results.successRate * 100).toFixed(
                                2,
                              )}
                              %
                            </span>
                          </div>
                          <br />
                        </div>
                        <div className="grid grid-cols-2 md:grid-cols-2 gap-x-2 gap-y-1 text-xs">
                          <div className="flex justify-between">
                            <span className="text-muted-foreground">
                              P95 Latency:
                            </span>
                            <span className="font-medium">
                              {hoveredPoint.results.p95_latency != null
                                ? `${hoveredPoint.results.p95_latency.toFixed(2)} ms`
                                : "N/A"}
                            </span>
                          </div>
                          <div className="flex justify-between">
                            <span className="text-muted-foreground">
                              Avg Latency:
                            </span>
                            <span className="font-medium">
                              {hoveredPoint.results.average != null
                                ? `${hoveredPoint.results.average.toFixed(2)} ms`
                                : "N/A"}
                            </span>
                          </div>
                        </div>

                        {/* Error distribution */}
                        {Object.keys(
                          hoveredPoint.results.errorDistribution || {},
                        ).length > 0 ? (
                          <div>
                            <div className="text-xs font-medium mb-1">
                              Error Distribution:
                            </div>
                            <div className="space-y-1">
                              {Object.entries(
                                hoveredPoint.results.errorDistribution || {},
                              ).map(([errorType, count]) => (
                                <div
                                  key={errorType}
                                  className="flex items-center justify-between text-xs"
                                >
                                  <span>{errorType}</span>
                                  <Badge
                                    variant="destructive"
                                    className="text-xs py-0"
                                  >
                                    {count}
                                  </Badge>
                                </div>
                              ))}
                            </div>
                          </div>
                        ) : (
                          <p className="text-xs text-muted-foreground">
                            No errors reported
                          </p>
                        )}
                      </div>
                    ) : (
                      <p className="text-xs text-muted-foreground">
                        Hover over a data point to see benchmark details
                      </p>
                    )}
                  </CardContent>
                </Card>
              </div>
              {topPerformers.length > 0 && (
                <div className="lg:col-span-5">
                  <Card className="shadow-sm h-full">
                    <CardHeader className="py-1 px-3">
                      <CardTitle className="text-sm flex items-center">
                        Top Performers{" "}
                        {filteredData[0] && (filteredData[0] as any).group && (
                          <span className="ml-1 text-xs text-muted-foreground">
                            ({(filteredData[0] as any).group})
                          </span>
                        )}
                      </CardTitle>
                    </CardHeader>
                    <CardContent className="p-2">
                      <div className="grid grid-cols-2 gap-x-3 gap-y-1">
                        {topPerformers.map(({ server, count }) => (
                          <div
                            key={server}
                            className="flex items-center justify-between text-xs"
                          >
                            <div className="flex items-center gap-1">
                              <div
                                className="w-2 h-2 rounded-full"
                                style={{
                                  backgroundColor: serverColors[server],
                                }}
                              />
                              <span className="truncate">
                                {formatServerName(server)}
                              </span>
                            </div>
                            <span className="text-muted-foreground ml-1">
                              {count}
                            </span>
                          </div>
                        ))}
                      </div>
                      {(filteredData[0] as any).group == "rack" && (
                        <p className="text-[10px] text-muted-foreground mt-2 px-1 leading-snug">
                          Note: Some servers (e.g. Unicorn, Agoo) don’t
                          participate in multi-threaded test cases so will
                          appear less frequently in these results.
                        </p>
                      )}
                    </CardContent>
                  </Card>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
