# Change: Developer Platform Experience (Backstage-like Features)

## Why

While the backend infrastructure (Aeterna) is powerful, it lacks a unified "Pane of Glass" for the 300+ engineers. Engineers currently interact via CLI or Git, which is fine for power users but insufficient for discovery and high-level governance.
Engineers often ask:
- "Who owns this service?"
- "What is the API for the Payment Service?"
- "Is my service compliant with security policies?"
- "How do I request a new repo?"

To truly enable a "Golden Path", Aeterna needs a **Developer Portal** (Web UI) and **Self-Service Workflows**.

## What Changes

### Major Features

#### 1. The Aeterna Portal (Web UI)
- **Tech Stack**: Next.js + Tailwind CSS (hosting the existing API).
- **Service Catalog**:
    - List of all synced Projects/Repos.
    - Filters by Owner (Team), Language, Lifecycle (Experimental, Production, Deprecated).
    - "My Dashboard": Shows "My Teams", "My Repos", "Pending Reviews".
- **Graph Explorer**:
    - Interactive visualization of the `GraphEdge` data (Service A -> Depends On -> Service B).
    - "Blast Radius" visualization: "If I change this library, who breaks?"

#### 2. Service Scorecards (Quality Standards)
- **Definition**: Define "Bronze", "Silver", "Gold" standards in `aeterna.yaml`.
    - e.g., Gold = "Has CODEOWNERS", "Has CI", "Test Coverage > 80%", "No Critical Vulns".
- **Scoring Engine**:
    - Periodic job runs checks against the repository metadata and index.
    - Scores are displayed on the Portal.
- **Gamification**: Leaderboards for Teams with highest adherence.

#### 3. Policy Playground & Simulation
- **"What If" Mode**:
    - UI to test Cedar policies without deploying them.
    - "Can user `alice` delete repo `payment-api`?" -> Result: `Deny (Policy: PreventDeletionOfProdRepos)`.
- **Policy CI**:
    - A mechanism to run regression tests on policy changes.

#### 4. Scaffold & Create (Self-Service)
- **Templating Engine**:
    - Upload "Golden Path" templates (e.g., `go-microservice`, `react-frontend`) to Aeterna.
- **Wizard**:
    - Engineers select a template in the Portal.
    - Aeterna creates the repo (via GitHub App), sets up CI, adds `CODEOWNERS`, and registers it in the Catalog.

### API Additions

- `GET /portal/catalog`: search/filter services.
- `GET /portal/scorecard/{repo_id}`: get compliance details.
- `POST /portal/scaffold`: trigger template generation.
- `POST /policy/simulate`: test inputs against Cedar engine.

## Impact

- **Discovery**: Engineers spend less time searching Slack/Wiki to find owners or docs.
- **Quality**: Scorecards drive behavioral change (everyone wants "Gold" status).
- **Velocity**: Scaffolding new services takes minutes, not days, and they are correct by default.

## Success Metrics

- 50% reduction in "Who owns this?" questions in Slack.
- 80% of Production services achieving "Silver" standard within 6 months.
- < 5 minutes to bootstrap a fully compliant new microservice.
