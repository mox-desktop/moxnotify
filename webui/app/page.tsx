import { Suspense } from "react"
import { NotificationDashboard } from "@/components/notification-dashboard"
import { Loader2 } from "lucide-react"

function LoadingFallback() {
  return (
    <div className="flex items-center justify-center min-h-screen">
      <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
    </div>
  )
}

export default function Home() {
  // Read API URL from environment variable at runtime (server-side)
  // Default matches searcher's default address (0.0.0.0:64203) but uses localhost for HTTP connection
  const apiBaseUrl = process.env.MOXNOTIFY_SEARCHER_ADDRESS || "http://localhost:64203"
  
  return (
    <main className="min-h-screen bg-background">
      <Suspense fallback={<LoadingFallback />}>
        <NotificationDashboard apiBaseUrl={apiBaseUrl} />
      </Suspense>
    </main>
  )
}
