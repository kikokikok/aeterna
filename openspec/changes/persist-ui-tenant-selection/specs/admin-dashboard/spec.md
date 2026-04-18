## MODIFIED Requirements

### Requirement: Tenant Selector
The admin UI SHALL provide a tenant selector in the shell header that enables the authenticated user to choose an active tenant. The selector SHALL bootstrap from the server-side default tenant preference on load and SHALL, by default, persist the user's choice back to the server so it is available on other devices.

#### Scenario: Bootstrap from server default
- **WHEN** the admin UI loads after a successful login
- **AND** the `GET /api/v1/auth/session` response includes `defaultTenantSlug: "<slug>"`
- **AND** no local override exists in `sessionStorage`
- **THEN** the UI SHALL set the active tenant to that slug and render it in the selector

#### Scenario: Persist selection across devices (default)
- **WHEN** the user selects a tenant in the `TenantSelector`
- **AND** the `Remember across devices` toggle is checked (default)
- **THEN** the UI SHALL call `PUT /api/v1/user/me/default-tenant { slug }` in addition to updating local state
- **AND** on success, the UI SHALL display the new active tenant immediately
- **AND** on failure, the UI SHALL roll back the local state and show a toast error

#### Scenario: Session-only selection
- **WHEN** the user selects a tenant with `Remember across devices` unchecked
- **THEN** the UI SHALL update only its in-browser state and SHALL NOT call the server default-tenant endpoint

#### Scenario: Clear active tenant (PlatformAdmin only)
- **WHEN** a PlatformAdmin chooses `Clear active tenant` in the selector menu
- **THEN** the UI SHALL remove the `X-Target-Tenant-ID` header from subsequent calls
- **AND** if `Remember across devices` is checked, the UI SHALL call `DELETE /api/v1/user/me/default-tenant`
- **AND** the selector SHALL display `Platform scope` (no tenant)

### Requirement: Select-Tenant Banner
When any UI-initiated API call receives `400 select_tenant`, the UI SHALL render a sticky banner allowing the user to pick a tenant from the `availableTenants` payload and SHALL retry the pending request with the chosen tenant applied, without requiring the user to navigate anywhere.

#### Scenario: Banner triggered on tenant-scoped request
- **WHEN** an API call returns `400 select_tenant` with `availableTenants: [...]`
- **THEN** the UI SHALL render `SelectTenantBanner` at the top of the current view
- **AND** the banner SHALL show a dropdown populated from `availableTenants`, the `hint` as a tooltip, and a `[Select]` button

#### Scenario: Selection retries pending request
- **WHEN** the user picks a tenant and clicks `[Select]` in the banner
- **THEN** the UI SHALL set the active tenant to the selected slug
- **AND** retry the original pending request with `X-Target-Tenant-ID: <chosen-slug>`
- **AND** the banner SHALL disappear on success

#### Scenario: Multiple pending requests coalesce
- **WHEN** more than one request returns `400 select_tenant` before the user picks
- **THEN** the UI SHALL show a single banner
- **AND** the single selection SHALL resolve all pending requests with the chosen tenant

### Requirement: Tenant Source Label
The UI SHALL indicate the source of the currently active tenant so users can understand why a given tenant is selected and how to change it.

#### Scenario: Source label renders next to selector
- **WHEN** the UI renders the tenant selector
- **THEN** a muted label next to the selector SHALL show one of: `from server default`, `session selection`, `single membership`, `admin impersonation`, `explicit selection`
- **AND** hovering the label SHALL show a tooltip explaining the source
