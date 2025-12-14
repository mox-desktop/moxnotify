"use client"

import { useState, useMemo, useEffect } from "react"
import { useSearchParams, useRouter, usePathname } from "next/navigation"
import { NotificationChart } from "./notification-chart"
import { NotificationList } from "./notification-list"
import { SearchBar } from "./search-bar"
import { FilterSidebar, type FilterState } from "./filter-sidebar"
import type { DbusNotification } from "@/lib/types"
import { mockNotifications } from "@/lib/mock-data"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Label } from "@/components/ui/label"

export function NotificationDashboard() {
  const searchParams = useSearchParams()
  const router = useRouter()
  const pathname = usePathname()

  // Parse filter state from URL
  const parseFiltersFromUrl = (): FilterState => {
    const appNames = searchParams.get("appNames")?.split(",").filter(Boolean) || []
    const hosts = searchParams.get("hosts")?.split(",").filter(Boolean) || []
    const urgencyParam = searchParams.get("urgency")
    const urgency = urgencyParam ? (Number(urgencyParam) as 0 | 1 | 2) : null
    const dateFrom = searchParams.get("dateFrom") || ""
    const dateTo = searchParams.get("dateTo") || ""

    return { appNames, hosts, urgency, dateFrom, dateTo }
  }

  // Initialize state from URL parameters
  const [searchInput, setSearchInput] = useState(searchParams.get("q") || "")
  const [searchQuery, setSearchQuery] = useState(searchParams.get("q") || "")
  const [notifications] = useState<DbusNotification[]>(mockNotifications)
  const [timeInterval, setTimeInterval] = useState(
    Number(searchParams.get("interval")) || 15
  )
  const [maxHits, setMaxHits] = useState<number | "all">(
    (searchParams.get("maxHits") === "all" || !searchParams.get("maxHits"))
      ? "all"
      : Number(searchParams.get("maxHits"))
  )
  const [filters, setFilters] = useState<FilterState>(parseFiltersFromUrl)

  // Update URL when state changes
  useEffect(() => {
    const params = new URLSearchParams()

    if (searchQuery.trim()) {
      params.set("q", searchQuery)
    }

    if (timeInterval !== 15) {
      params.set("interval", timeInterval.toString())
    }

    if (maxHits !== "all") {
      params.set("maxHits", maxHits.toString())
    }

    if (filters.appNames.length > 0) {
      params.set("appNames", filters.appNames.join(","))
    }

    if (filters.hosts.length > 0) {
      params.set("hosts", filters.hosts.join(","))
    }

    if (filters.urgency !== null) {
      params.set("urgency", filters.urgency.toString())
    }

    if (filters.dateFrom) {
      params.set("dateFrom", filters.dateFrom)
    }

    if (filters.dateTo) {
      params.set("dateTo", filters.dateTo)
    }

    const newUrl = params.toString() ? `${pathname}?${params.toString()}` : pathname
    router.replace(newUrl, { scroll: false })
  }, [searchQuery, timeInterval, maxHits, filters, pathname, router])

  const handleRunSearch = () => {
    setSearchQuery(searchInput)
  }

  const handleTimeIntervalChange = (value: string) => {
    setTimeInterval(Number(value))
  }

  const handleMaxHitsChange = (value: string) => {
    setMaxHits(value === "all" ? "all" : Number(value))
  }

  const filteredNotifications = useMemo(() => {
    let filtered = notifications

    // Apply text search query
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase()
      filtered = filtered.filter(
        (notification) =>
          notification.app_name.toLowerCase().includes(query) ||
          notification.summary.toLowerCase().includes(query) ||
          notification.body.toLowerCase().includes(query) ||
          notification.host.toLowerCase().includes(query),
      )
    }

    // Apply field filters
    if (filters.appNames.length > 0) {
      filtered = filtered.filter((notification) => filters.appNames.includes(notification.app_name))
    }

    if (filters.hosts.length > 0) {
      filtered = filtered.filter((notification) => filters.hosts.includes(notification.host))
    }

    if (filters.urgency !== null) {
      filtered = filtered.filter((notification) => notification.urgency === filters.urgency)
    }

    if (filters.dateFrom) {
      const fromDate = new Date(filters.dateFrom)
      filtered = filtered.filter((notification) => notification.timestamp >= fromDate)
    }

    if (filters.dateTo) {
      const toDate = new Date(filters.dateTo)
      filtered = filtered.filter((notification) => notification.timestamp <= toDate)
    }

    // Apply max hits limit
    if (maxHits !== "all") {
      filtered = filtered.slice(0, maxHits)
    }

    return filtered
  }, [notifications, searchQuery, filters, maxHits])

  return (
    <div className="flex h-screen overflow-hidden">
      {/* Filter Sidebar */}
      <div className="flex-shrink-0">
        <FilterSidebar notifications={notifications} filters={filters} onFiltersChange={setFilters} />
      </div>

      {/* Main Content */}
      <div className="flex-1 overflow-y-auto">
        <div className="mx-auto max-w-7xl p-4 md:p-6 lg:p-8">
          <header className="mb-6">
            <h1 className="text-3xl font-bold tracking-tight text-foreground">Notification Center</h1>
            <p className="mt-1 text-sm text-muted-foreground">D-Bus specification compliant notification viewer</p>
          </header>

          <div className="space-y-6">
            <div className="flex flex-wrap items-center gap-3">
              <div className="flex items-center gap-3">
                <Label htmlFor="interval-select" className="text-sm font-medium text-foreground">
                  Chart Time Interval:
                </Label>
                <Select value={timeInterval.toString()} onValueChange={handleTimeIntervalChange}>
                  <SelectTrigger id="interval-select" className="w-[180px]">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="5">5 minutes</SelectItem>
                    <SelectItem value="10">10 minutes</SelectItem>
                    <SelectItem value="15">15 minutes</SelectItem>
                    <SelectItem value="20">20 minutes</SelectItem>
                    <SelectItem value="30">30 minutes</SelectItem>
                    <SelectItem value="60">1 hour</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="flex items-center gap-3">
                <Label htmlFor="max-hits-select" className="text-sm font-medium text-foreground">
                  Max Hits:
                </Label>
                <Select
                  value={maxHits === "all" ? "all" : maxHits.toString()}
                  onValueChange={handleMaxHitsChange}
                >
                  <SelectTrigger id="max-hits-select" className="w-[180px]">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="10">10</SelectItem>
                    <SelectItem value="25">25</SelectItem>
                    <SelectItem value="50">50</SelectItem>
                    <SelectItem value="100">100</SelectItem>
                    <SelectItem value="all">All</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>

            <NotificationChart notifications={notifications} timeInterval={timeInterval} />
            <SearchBar value={searchInput} onChange={setSearchInput} onRun={handleRunSearch} />
            <NotificationList notifications={filteredNotifications} />
          </div>
        </div>
      </div>
    </div>
  )
}
