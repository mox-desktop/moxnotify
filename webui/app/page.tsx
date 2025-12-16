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
  return (
    <main className="min-h-screen bg-background">
      <Suspense fallback={<LoadingFallback />}>
        <NotificationDashboard />
      </Suspense>
    </main>
  )
}
