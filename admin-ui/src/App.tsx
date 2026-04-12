import { createBrowserRouter, RouterProvider } from "react-router-dom"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { AuthProvider } from "@/auth/AuthContext"
import { ProtectedRoute } from "@/auth/ProtectedRoute"
import { AdminLayout } from "@/layouts/AdminLayout"
import LoginPage from "@/auth/LoginPage"
import DashboardPage from "@/pages/dashboard/DashboardPage"

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 1,
    },
  },
})

const router = createBrowserRouter(
  [
    { path: "/admin/login", element: <LoginPage /> },
    {
      path: "/admin",
      element: (
        <ProtectedRoute>
          <AdminLayout />
        </ProtectedRoute>
      ),
      children: [
        { index: true, element: <DashboardPage /> },
        {
          path: "tenants",
          lazy: () => import("./pages/tenants/TenantListPage"),
        },
        {
          path: "tenants/:id",
          lazy: () => import("./pages/tenants/TenantDetailPage"),
        },
        {
          path: "organizations",
          lazy: () => import("./pages/organizations/OrgTreePage"),
        },
        {
          path: "users",
          lazy: () => import("./pages/users/UserListPage"),
        },
        {
          path: "users/:id",
          lazy: () => import("./pages/users/UserDetailPage"),
        },
        {
          path: "knowledge",
          lazy: () => import("./pages/knowledge/KnowledgeSearchPage"),
        },
        {
          path: "knowledge/:id",
          lazy: () => import("./pages/knowledge/KnowledgeDetailPage"),
        },
        {
          path: "memory",
          lazy: () => import("./pages/memory/MemorySearchPage"),
        },
        {
          path: "governance",
          lazy: () => import("./pages/governance/PendingRequestsPage"),
        },
        {
          path: "governance/audit",
          lazy: () => import("./pages/governance/AuditLogPage"),
        },
        {
          path: "policies",
          lazy: () => import("./pages/policies/PolicyListPage"),
        },
        {
          path: "admin",
          lazy: () => import("./pages/admin/SystemHealthPage"),
        },
        {
          path: "admin/lifecycle",
          lazy: async () => {
            const mod = await import("./pages/admin/LifecyclePage")
            return { Component: mod.default }
          },
        },
        {
          path: "settings",
          lazy: () => import("./pages/settings/ServerConfigPage"),
        },
      ],
    },
  ],
  { basename: "/" },
)

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AuthProvider>
        <RouterProvider router={router} />
      </AuthProvider>
    </QueryClientProvider>
  )
}
