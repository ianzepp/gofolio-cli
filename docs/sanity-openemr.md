# Sanity Check: OpenEMR as AgentForge Source Repository

## Project Identity

| Attribute        | Value                                                                       |
| ---------------- | --------------------------------------------------------------------------- |
| Project          | OpenEMR — open-source electronic health records                             |
| Primary Language | PHP (8.2+)                                                                  |
| Frontend         | jQuery 3.7, Bootstrap 4.6, Twig/Smarty/Mustache templates (server-rendered) |
| Database         | MariaDB 11.8 via ADODB wrapper + Doctrine DBAL                              |
| License          | GNU GPL v3                                                                  |
| Maturity         | 20+ years, active development, 27 CI workflows                              |

---

## Project Scale

| Metric                         | Value                                              |
| ------------------------------ | -------------------------------------------------- |
| Total PHP files                | 4,272                                              |
| Total JS files                 | 394                                                |
| Total Twig templates           | 268                                                |
| PHP lines of code              | ~331,000                                           |
| JS lines of code               | ~228,000                                           |
| Database tables                | 281 (`CREATE TABLE` statements)                    |
| SQL schema file                | 15,402 lines                                       |
| Repo on disk                   | 1.1 GB                                             |
| Composer packages (production) | ~85                                                |
| NPM packages (production)      | 43 + 8 napa packages                               |
| Required PHP extensions        | 28 (including native: imagick, ldap, intl, sodium) |

### Source directory breakdown

| Directory     | PHP files | Role                         |
| ------------- | --------- | ---------------------------- |
| `/src/`       | 1,867     | Modern PSR-4 namespace code  |
| `/library/`   | 598       | Legacy procedural PHP        |
| `/interface/` | 1,006     | Web UI controllers/templates |
| `/tests/`     | 294       | Test suite                   |

---

## API Surface

### Standard REST API

- **92 route handler definitions** across patient, encounter, vitals, medications, appointments, practitioners, facilities, insurance, documents, etc.
- **OAuth 2.0 + SMART on FHIR** authentication required for all endpoints
- Swagger/OpenAPI documentation (10,048-line YAML spec) with Swagger UI included

| Category            | Endpoints                                                     | Methods                |
| ------------------- | ------------------------------------------------------------- | ---------------------- |
| Patient             | `/api/patient`, `/api/patient/:puuid`                         | GET, POST, PUT         |
| Appointments        | `/api/appointment`, `/api/patient/:pid/appointment`           | GET, POST, DELETE      |
| Practitioner        | `/api/practitioner`, `/api/practitioner/:pruuid`              | GET, POST, PUT         |
| Drugs               | `/api/drug`, `/api/drug/:uuid`                                | GET only               |
| Prescriptions       | `/api/prescription`, `/api/prescription/:uuid`                | GET only               |
| Medications         | `/api/patient/:pid/medication`                                | GET, POST, PUT, DELETE |
| Medical Problems    | `/api/medical_problem`, `/api/patient/:puuid/medical_problem` | GET, POST, PUT, DELETE |
| Allergies           | `/api/allergy`, `/api/patient/:puuid/allergy`                 | GET, POST, PUT, DELETE |
| Insurance           | `/api/patient/:puuid/insurance`                               | GET, POST, PUT         |
| Insurance Companies | `/api/insurance_company`                                      | GET, POST, PUT         |
| Encounters          | `/api/patient/:puuid/encounter`                               | GET, POST, PUT         |
| Vitals              | `/api/patient/:pid/encounter/:eid/vital`                      | GET, POST, PUT         |
| Immunizations       | `/api/immunization`                                           | GET only               |

### FHIR R4 API (US Core compliant)

- **80 route handler definitions** covering **36 FHIR resource types**
- Includes bulk export ($export), ValueSet lookup, OperationDefinition
- Write support limited to: Organization, Patient, Practitioner (POST + PUT)
- All other FHIR resources are read-only

Key FHIR resources: AllergyIntolerance, Appointment, CarePlan, CareTeam, Condition, Coverage, Device, DiagnosticReport, DocumentReference, Encounter, Goal, Immunization, Location, Medication, MedicationRequest, MedicationDispense, Observation, Organization, Patient, Practitioner, PractitionerRole, Procedure, ServiceRequest, Specimen, ValueSet

### Portal API

- **5 read-only routes** for patient self-service (own record, encounters, appointments)

---

## Service Layer

25 standard REST controllers + 35 FHIR controllers, backed by ~50 services in `src/Services/`. All services extend `BaseService`. Key services relevant to the assignment:

| Service                     | DB Table(s)                    | API Exposure  |
| --------------------------- | ------------------------------ | ------------- |
| `DrugService`               | `drugs`, `drug_inventory`      | REST GET only |
| `PrescriptionService`       | `prescriptions`                | REST GET only |
| `AppointmentService`        | `openemr_postcalendar_events`  | REST + FHIR   |
| `PractitionerService`       | `users`                        | REST + FHIR   |
| `InsuranceService`          | `insurance_data`               | REST + FHIR   |
| `ConditionService`          | `lists` (type=medical_problem) | REST + FHIR   |
| `AllergyIntoleranceService` | `lists` (type=allergy)         | REST + FHIR   |
| `PatientService`            | `patient_data`                 | REST + FHIR   |
| `ObservationService`        | `form_vitals`                  | REST + FHIR   |
| `FacilityService`           | `facility`                     | REST + FHIR   |

### Services with NO API exposure (internal only)

- `DecisionSupportInterventionService` — clinical decision support logic
- `CodeTypesService` — CPT/ICD/SNOMED/LOINC lookups
- `DrugSalesService` — dispensing records
- `PatientTrackerService` — patient flow tracking
- `FormService` — dynamic form definitions
- All billing/financial services
- All Globals/Settings services

---

## Mapping to AgentForge Required Tools

The assignment suggests 5 healthcare tools. Here is how each maps to OpenEMR:

### Tool 1: `drug_interaction_check(medications[])` → interactions, severity

| Aspect                      | Status                                                                        |
| --------------------------- | ----------------------------------------------------------------------------- |
| Data availability           | Drugs have RxNorm/RXCUI codes; prescriptions linked to patients               |
| Built-in interaction engine | **DOES NOT EXIST**                                                            |
| API endpoint                | `GET /api/prescription?patient={pid}` returns RXCUI codes                     |
| Required work               | Must integrate external NLM RxNav API (`rxnav.nlm.nih.gov/REST/interaction/`) |
| Verification feasibility    | Good — RxNav returns severity levels, can cross-check                         |

**Red flag:** This is the most commonly cited healthcare agent tool, but OpenEMR has zero interaction-checking logic. You are building this from scratch with an external API dependency.

### Tool 2: `symptom_lookup(symptoms[])` → possible conditions, urgency

| Aspect                                | Status                                                                                                                                     |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Data availability                     | Conditions stored with ICD-10 and SNOMED codes                                                                                             |
| Built-in symptom-to-condition mapping | **DOES NOT EXIST**                                                                                                                         |
| API endpoint                          | `GET /fhir/Condition` and `/api/patient/:pid/medical_problem` return coded conditions                                                      |
| Required work                         | OpenEMR stores _diagnosed_ conditions, not symptom-to-condition inference. You'd need an external clinical knowledge base or LLM reasoning |
| Verification feasibility              | Can verify against ICD-10 code descriptions                                                                                                |

**Red flag:** OpenEMR is a record-keeping system, not a diagnostic engine. "Symptom lookup" implies inference that doesn't exist in the data model. You'd be querying existing patient problem lists, not doing symptom-based differential diagnosis.

### Tool 3: `provider_search(specialty, location)` → available providers

| Aspect                   | Status                                                                                         |
| ------------------------ | ---------------------------------------------------------------------------------------------- |
| Data availability        | Practitioners with NPI, specialty, facility linkage                                            |
| API endpoint             | `GET /api/practitioner` with query params; `GET /fhir/PractitionerRole` for specialty+location |
| Required work            | Minimal — API surface is ready                                                                 |
| Verification feasibility | NPI numbers are verifiable against NPPES registry                                              |

**Green flag.** This is the most straightforward tool to implement.

### Tool 4: `appointment_availability(provider_id, date_range)` → slots

| Aspect                       | Status                                                                                                                         |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| Data availability            | Booked appointments with times, provider, status, categories                                                                   |
| Built-in availability engine | **DOES NOT EXIST**                                                                                                             |
| API endpoint                 | `GET /api/appointment` returns booked slots only                                                                               |
| Required work                | Must build gap-finding logic: query booked slots for a provider in a date range, compute open slots based on provider schedule |
| Verification feasibility     | Can verify against actual bookings                                                                                             |

**Yellow flag.** The data is there but you must build the availability calculation. You also need to know provider working hours/schedule, which may or may not be in the data.

### Tool 5: `insurance_coverage_check(procedure_code, plan_id)` → coverage details

| Aspect                             | Status                                                                                                                                                     |
| ---------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Data availability                  | Insurance records: plan name, policy number, copay, type (primary/secondary/tertiary)                                                                      |
| Real-time eligibility verification | **NOT REST-ACCESSIBLE** — EDI 270/271 exists (15,000 lines of X12 parsing code in `library/edihistory/`) but it's legacy, file-based, with no API endpoint |
| API endpoint                       | `GET /fhir/Coverage` and `/api/patient/:pid/insurance` return stored plan data only                                                                        |
| Required work                      | Can return what plan a patient has, but cannot verify if a specific procedure is covered. Would need external payer API integration                        |
| Verification feasibility           | Limited to what's stored in the EHR                                                                                                                        |

**Red flag.** "Coverage check" implies real-time eligibility verification, which is a complex healthcare-specific problem (EDI X12, payer APIs). OpenEMR's X12 subsystem is 15,000+ lines of legacy code that is not API-accessible. You'd likely have to simplify this tool to "return patient's insurance info" rather than actual coverage verification.

### Summary: Tool readiness

| Tool                     | Readiness                           | External Dependencies                     |
| ------------------------ | ----------------------------------- | ----------------------------------------- |
| Drug interaction check   | Low — no built-in logic             | NLM RxNav API required                    |
| Symptom lookup           | Low — wrong data model              | External knowledge base or LLM reasoning  |
| Provider search          | High — API ready                    | None                                      |
| Appointment availability | Medium — data exists, logic doesn't | Custom gap-finding algorithm              |
| Insurance coverage check | Low — stored data only              | External payer APIs for real verification |

**Only 1 of 5 suggested tools is ready out-of-the-box. 2 more are buildable with moderate effort. 2 require significant external integrations or scope reduction.**

Note: The assignment requires "minimum 5" tools but these 5 are _suggestions_, not mandates. You could substitute easier tools using other available API endpoints (e.g., allergy lookup, patient demographics search, immunization history, encounter history, facility search) to hit the 5-tool minimum faster, then tackle the harder tools as stretch goals.

---

## Docker / Development Environment

### Setup complexity

| Aspect                     | Detail                                                                                                    |
| -------------------------- | --------------------------------------------------------------------------------------------------------- |
| Docker services required   | **7**: MariaDB, OpenEMR (PHP/Apache), Selenium, phpMyAdmin, CouchDB, OpenLDAP, Mailpit                    |
| Named Docker volumes       | 10                                                                                                        |
| Cold start time            | ~5-10 minutes (MySQL healthcheck + OpenEMR init runs npm install, composer install, asset build, DB init) |
| Demo data seeding          | `docker compose exec openemr /root/devtools dev-reset-install-demodata`                                   |
| Synthea patient generation | `docker compose exec openemr /root/devtools import-random-patients 100`                                   |
| Recommended host resources | Ubuntu 22.04, 40GB disk, 25% host RAM, 25% host CPU                                                       |

### Gotchas

- **`vendor/` and `node_modules/` live in Docker named volumes, not on host disk.** This means IDE autocompletion and static analysis may not work without separate host-side installs.
- After any DB reset devtool command, **CouchDB must be manually restarted**.
- A **GitHub Composer token is hardcoded in plaintext** in docker-compose.yml.
- Xdebug is ON by default (performance impact).
- `napa` packages (8 packages fetched from GitHub archives) are re-downloaded on every `npm install` due to `"cache": false`.

### Isolated tests (no Docker)

Isolated tests run on the host without Docker: `composer phpunit-isolated`. 74 isolated test files exist. Requires local PHP 8.2+ with all 28 extensions installed.

---

## Testing Infrastructure

| Metric                      | Value                   |
| --------------------------- | ----------------------- |
| Total test files            | 294                     |
| Total test methods          | 2,244                   |
| Isolated (no DB) test files | 74                      |
| Unit test files             | 43                      |
| E2E test files              | 33                      |
| API test files              | 17                      |
| CI workflow files           | 27                      |
| CI matrix configs           | 17 Docker compose files |

- PHPUnit 11 with two configurations: isolated (host-only) and full (Docker-dependent)
- E2E uses Symfony Panther + Selenium Chromium
- Twig template compilation and render tests with fixture files
- **No code coverage reporting configured** (`<coverage>` block is empty in phpunit.xml)
- phpstan requires **4GB RAM**; baseline generation requires **8GB RAM**
- Full CI matrix tests PHP 8.2/8.3/8.4/8.5/8.6 × MariaDB 5.7 through 12.0 × Apache/Nginx

---

## Language and Stack Concerns

### PHP as the source repo language

The AgentForge assignment recommends:

- **Agent framework:** LangChain, LangGraph, CrewAI, AutoGen, Semantic Kernel — all Python or .NET
- **Backend:** Python/FastAPI or Node.js/Express
- **Frontend:** React, Next.js, or Streamlit

OpenEMR is **100% PHP backend** with jQuery/Bootstrap frontend. This means:

1. **The agent cannot live inside OpenEMR.** It must be a separate service (Python or Node) that calls OpenEMR's APIs.
2. **Three deployment targets.** You're running MariaDB + OpenEMR + the agent service (plus a TLS proxy for production).
3. **No shared code.** You can't import OpenEMR services directly into your agent. Everything goes through HTTP APIs.
4. **Open source contribution path is constrained.** Contributing PHP code to OpenEMR requires understanding the PHP codebase. Contributing the Python agent as a separate repo is simpler but arguably less integrated.

### Frontend considerations

- AngularJS 1.8 is used but only in one subsystem (questionnaire forms). Not an app-wide framework.
- The frontend is server-rendered PHP pages with jQuery progressive enhancement. **There is no SPA.**
- **Four template engines coexist:** Twig (268 files), Smarty (legacy), Mustache (104 files), plain PHP views (50 files).
- Building a new agent UI does not require touching any existing frontend code — just consume the REST/FHIR APIs.

### Database complexity

- **281 tables.** The schema is vast and decades old.
- Migration history spans OpenEMR versions 2.6.0 through 7.0.x.
- ADODB wrapper (legacy) + Doctrine DBAL (newer code) coexist.
- The `lists` table is a multi-purpose table storing medical problems, allergies, AND medications by `type` discriminator column — a legacy design pattern.

---

## Authentication Complexity

Getting an API token requires:

1. Enable APIs in the admin UI (3 checkboxes in Administration > Config > Connectors) — **pre-enabled in Docker dev via env vars**
2. Configure the SSL base URL in the admin UI — **pre-configured in Docker dev via `OPENEMR_SETTING_site_addr_oath`**
3. `POST /oauth2/default/registration` with JSON body → receive `client_id` and `client_secret`
4. `POST /oauth2/default/token` with `grant_type=password` + credentials → receive access token

- **HTTPS is mandatory** for all OAuth2 operations
- Password grant is available in dev mode (pre-enabled in Docker via `OPENEMR_SETTING_oauth_password_grant: 3`; must be manually enabled in production)
- Full SMART on FHIR v2.2.0 is implemented (asymmetric auth, PKCE, token introspection)
- Scope model: `{context}/{resource}.{permission}` (e.g., `user/Patient.read`)
- **No rate limiting exists anywhere**
- CORS allows **any origin** with a TODO comment acknowledging the security issue

**Red flag for agent development:** The OAuth2 setup adds friction to every API call during development. You'll need to automate token acquisition in your agent, handle token refresh, and manage scopes per tool.

---

## Pitfalls and Red Flags Summary

### Critical

| #   | Issue                                                                                                                    | Impact                                                                                                                       |
| --- | ------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| 1   | **PHP codebase, Python/Node agent** — complete language mismatch with recommended agent frameworks                       | Two separate systems to deploy and maintain; no code sharing; integration only through HTTP APIs                             |
| 2   | **Only 1 of 5 suggested tools is API-ready** — most suggested tools require external service integration or custom logic | Significant plumbing time if you build the suggested tools; mitigated by substituting easier tools from the rich API surface |
| 3   | **OAuth2 setup complexity** — multi-step token acquisition, HTTPS-only, scoped permissions                               | Agent must handle auth flow, token refresh, and correct scopes; adds development friction                                    |
| 4   | **7-service Docker stack** — heavy local development requirements                                                        | 5-10 min cold start, 25% host RAM recommended, manual CouchDB restart after DB resets                                        |

### Significant

| #   | Issue                                                                                                              | Impact                                                       |
| --- | ------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------ |
| 5   | **No drug interaction engine** — the flagship healthcare tool requires building from scratch with external NLM API | Most demo-worthy feature requires the most work              |
| 6   | **No availability engine** — appointments exist but slot-finding doesn't                                           | Must reverse-engineer scheduling logic or build from scratch |
| 7   | **EDI/X12 not API-accessible** — 15,000 lines of insurance processing code locked in legacy PHP                    | Can't meaningfully do insurance verification through the API |
| 8   | **4GB+ RAM for static analysis** — phpstan/rector memory requirements are unusually high                           | Host machine constraints for code quality tooling            |
| 9   | **281-table schema** — massive database with decades of migration history                                          | Steep learning curve for understanding data relationships    |
| 10  | **28 required PHP extensions** — including native extensions (imagick, ldap, intl)                                 | Complex host setup outside Docker; IDE support may suffer    |

### Minor but notable

| #   | Issue                                                              | Impact                                                                             |
| --- | ------------------------------------------------------------------ | ---------------------------------------------------------------------------------- |
| 11  | **vendor/ and node_modules/ in Docker volumes** — not on host disk | IDE autocompletion and goto-definition may not work without separate host installs |
| 12  | **No `strict_types`** — project-wide convention                    | Type safety relies on phpstan rather than runtime checks                           |
| 13  | **Four coexisting template engines**                               | Cognitive overhead if you need to touch any UI                                     |
| 14  | **CORS allows any origin**                                         | Not a problem for the assignment but a security concern in production              |
| 15  | **No rate limiting**                                               | Agent could overwhelm the API during rapid development/testing                     |

---

## Strengths

| #   | Strength                                                                    | Relevance                                                                        |
| --- | --------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| 1   | **Mature, well-documented REST + FHIR APIs** with Swagger UI                | Clean integration surface for an external agent                                  |
| 2   | **Standard medical coding throughout** (RxNorm, ICD-10, SNOMED, LOINC)      | Enables meaningful verification and fact-checking against standard terminologies |
| 3   | **FHIR R4 US Core compliance** with 36 resource types                       | Industry-standard healthcare API; transferable knowledge; impressive for demos   |
| 4   | **Demo data and Synthea patient generation** built into devtools            | Can populate realistic test data quickly                                         |
| 5   | **Active project with strong CI** (27 workflows, multi-version matrix)      | Real open-source project; contribution has genuine impact                        |
| 6   | **Domain prestige** — healthcare EMR is a high-stakes, regulated domain     | Aligns perfectly with the assignment's emphasis on verification and safety       |
| 7   | **Large contributor community** — established PR workflow and conventions   | Open source contribution path is well-defined                                    |
| 8   | **API-first architecture is viable** — agent doesn't need to touch PHP code | Clean separation of concerns between agent and data source                       |

---

## Open Source Contribution Paths

| Path                                         | Feasibility | Notes                                                                              |
| -------------------------------------------- | ----------- | ---------------------------------------------------------------------------------- |
| New API endpoints in OpenEMR (PHP)           | Medium      | Requires PHP expertise; could add drug interaction endpoint, availability endpoint |
| Agent as separate published package (Python) | High        | Publish to PyPI; cleanest path but less integrated with OpenEMR                    |
| Eval dataset for healthcare agents           | High        | 50+ test cases is a genuine community contribution                                 |
| FHIR tooling/documentation                   | Medium      | Improve FHIR documentation or add missing endpoints                                |

---

## Integration Surface Area: What Do You Actually Have to Touch?

### Answer: zero lines of PHP

The agent is a separate Python/Node service. All data access is through OpenEMR's existing REST and FHIR APIs over HTTP. You never need to read, understand, or modify the 331,000 lines of PHP, the 281-table schema, or the build pipeline. The integration surface is:

1. An HTTP client in your agent
2. OAuth2 token management (one-time setup + token refresh logic)
3. Knowledge of which endpoints to call and what they return

### API response quality by tool

| Tool                | Endpoint                                               | Fields returned                                                                          | Sufficient for agent?                                                            |
| ------------------- | ------------------------------------------------------ | ---------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| Provider search     | `GET /api/practitioner?specialty=X`                    | Name, NPI, specialty, facility, contact, taxonomy code                                   | Yes — ready to use                                                               |
| Patient conditions  | `GET /fhir/Condition?patient={uuid}`                   | ICD-10/SNOMED codes with descriptions, onset/end dates, verification status, provider    | Yes — ready to use                                                               |
| Patient medications | `GET /api/prescription?patient={uuid}`                 | Drug name, RxNorm/RXCUI code + description, dosage, route, interval, adherence, provider | Yes — ready to use                                                               |
| Patient insurance   | `GET /fhir/Coverage?patient={uuid}`                    | Plan name, policy/group numbers, copay, COB order, effective dates, status               | Mostly — insurer name requires follow-up `GET /fhir/Organization/{uuid}`         |
| Appointments        | `GET /api/patient/{pid}/appointment`                   | Date, time, duration, provider name/NPI, facility, status code                           | Mostly — status is raw codes (`^`, `x`, `-`), need a mapping table in your agent |
| Drug interactions   | `GET /api/prescription?patient={uuid}` → external call | RxCUI codes from OpenEMR; interaction data from NLM RxNav                                | No — must call `rxnav.nlm.nih.gov/REST/interaction/` externally                  |
| Slot availability   | `GET /api/appointment` filtered by provider + date     | Booked slots only                                                                        | No — must build gap-finding logic in your agent                                  |

### OAuth2 setup (the one piece of friction)

Before any API call works:

1. **Admin UI** — enable APIs + password grant (3 checkboxes in Administration > Config > Connectors) — **already done in Docker dev**
2. **Register client** — `POST /oauth2/default/registration` with JSON body → receive `client_id` + `client_secret`
3. **Get token** — `POST /oauth2/default/token` with `grant_type=password` → 1-hour bearer token
4. **Refresh** — tokens expire in 1 hour; refresh tokens last 3 months with `offline_access` scope

HTTPS is mandatory even in dev (Docker pre-configures port 9300). This is automatable boilerplate you write once.

### Response format

**Standard API** returns:

```json
{
    "validationErrors": [],
    "internalErrors": [],
    "data": [ ... ],
    "links": [ { "self": "...", "next": "..." } ]
}
```

**FHIR API** returns standard FHIR R4 Bundle/Resource JSON:

```json
{
    "resourceType": "Bundle",
    "type": "searchset",
    "entry": [ { "fullUrl": "...", "resource": { ... } } ]
}
```

Both are clean JSON. Pagination is via `_limit` and `_offset` query params (standard API) or FHIR Bundle links.

### What's NOT exposed via API (gaps that might force you into PHP)

| Gap                                                     | Workaround without touching PHP                                                          |
| ------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| Drug name search is exact-match only                    | Fuzzy match in your agent, or use FHIR `Medication?code:text=`                           |
| Appointment status codes are raw (`^`, `x`, `-`)        | Hardcode the small fixed mapping in your agent                                           |
| No available-slots endpoint                             | Compute in your agent from booked appointments + assumed schedule                        |
| Insurance company name not joined in standard API       | One extra `GET` per unique insurer UUID                                                  |
| No drug interaction engine                              | External NLM RxNav API call with RXCUI codes from prescriptions                          |
| `CodeTypesService` (ICD/SNOMED/CPT lookups) not exposed | Use `GET /fhir/ValueSet` or use coded data returned inline with conditions/prescriptions |
| `DecisionSupportInterventionService` not exposed        | Not usable — clinical decision support is internal only                                  |
| All billing/financial services not exposed              | Not usable via API                                                                       |

**None of these gaps require modifying PHP.** They all have workarounds in the agent layer or via external APIs. The only scenario that would force you into the PHP codebase is if you wanted to add new API endpoints as an open-source contribution — which is optional, not required for the agent to function.

---

## Testing Situation: Existing Tests and Eval Blockers

### What OpenEMR already has

| Suite                           | Files   | Test methods | Needs Docker? | Needs DB? | Needs HTTP?    |
| ------------------------------- | ------- | ------------ | ------------- | --------- | -------------- |
| Isolated (phpunit-isolated.xml) | 74      | —            | No            | No        | No             |
| Unit (phpunit.xml)              | 43      | —            | Yes           | Yes       | No             |
| Services (phpunit.xml)          | —       | —            | Yes           | Yes       | No             |
| API (phpunit.xml)               | 17      | —            | Yes           | Yes       | Yes (Guzzle)   |
| E2E (phpunit.xml)               | 33      | —            | Yes           | Yes       | Yes (Selenium) |
| Jest (JS)                       | 2       | —            | No            | No        | No             |
| **Total**                       | **294** | **2,244**    | —             | —         | —              |

**None of these tests are relevant to your agent evals.** They test PHP internals — service methods, validators, UI flows. Your eval tests will test whether your LLM agent correctly selects tools, passes correct parameters, and synthesizes coherent responses. Completely different test target.

### What you actually need for your 50+ eval test cases

Your evals test the **agent**, not OpenEMR. But the agent calls OpenEMR's API, so the eval environment needs:

1. **A running OpenEMR with test data** — patients who have medications, conditions, appointments, and insurance
2. **Deterministic data** — same eval run twice should hit the same patient data
3. **API stability** — tokens don't expire mid-run, Docker stack stays up

### Test data situation

| Data source                          | What you get                            | Clinical data?                                                                                      | Deterministic?            |
| ------------------------------------ | --------------------------------------- | --------------------------------------------------------------------------------------------------- | ------------------------- |
| `dev-reset-install-demodata`         | 14 patients + pre-configured users/ACLs | **No** — demographics only, zero clinical records                                                   | Yes — same every time     |
| `import-random-patients N` (Synthea) | N patients with full histories          | **Yes** — encounters, vitals, prescriptions, allergies, conditions, immunizations, procedures, labs | **No** — random every run |
| `example_patient_data.sql`           | 14 patients (same as demodata)          | **No** — demographics only                                                                          | Yes                       |

**This is a problem.** The deterministic data source has no clinical data. The clinical data source is random every time. For reproducible eval tests, you would need to either:

- Run Synthea once, then snapshot the database state and restore it for each eval run
- Create your own fixture data (INSERT via the API or SQL) with known patients, known medications, known conditions
- Accept non-deterministic data and write evals that test agent behavior patterns rather than exact outputs (e.g., "agent correctly identifies a drug interaction" rather than "agent returns exactly these 3 interactions")

### Blockers and risks for LLM eval tests

| Blocker                                  | Severity   | Detail                                                                                                                                                                                           |
| ---------------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **No clinical demo data out of the box** | High       | Demo install gives you 14 patients with names and addresses but zero medications, conditions, appointments, or insurance. You must seed clinical data yourself or run Synthea (which is random). |
| **OAuth2 token expiry during eval runs** | Medium     | Tokens expire in 1 hour. A 50-case eval suite with LLM calls could take longer. Must implement token refresh or request `offline_access` for 3-month refresh tokens.                             |
| **Docker stack stability**               | Medium     | Docker stack must stay running for entire eval suite (can be stripped to 2-3 services for agent dev). If MariaDB crashes under load, evals fail mid-run.                                         |
| **No rate limiting = no backpressure**   | Low-Medium | No rate limiting on the API means your eval harness won't be throttled, but it also means you could accidentally overload the single-instance Docker setup with concurrent requests.             |
| **Synthea import speed**                 | Low        | Each Synthea patient takes several seconds to import. Generating 100 patients = several minutes of setup before evals can start.                                                                 |
| **HTTPS certificate in dev**             | Low        | Docker uses a self-signed cert on port 9300. Your Python test client needs to either disable TLS verification or trust the cert. Minor but annoying.                                             |

### What an eval test case looks like in practice

Your eval harness is a **Python test suite** that:

1. Sends a natural language query to your agent (e.g., "What medications is patient John Smith taking?")
2. Your agent decides to call `GET /api/prescription?patient={uuid}` on OpenEMR
3. OpenEMR returns structured JSON with medication data
4. Your agent synthesizes a response
5. Your eval checks: Did the agent pick the right tool? Did it pass correct parameters? Is the response accurate?

The eval tests OpenEMR only indirectly — as the data backend. The actual assertions are on agent behavior: tool selection, parameter correctness, response quality, hallucination detection.

### Can you run evals without Docker/OpenEMR at all?

Yes — if you **mock the API responses.** You could:

- Record real API responses from OpenEMR once (fixture capture)
- Replay them in your eval harness without a running OpenEMR instance
- This is faster, deterministic, and doesn't require Docker during eval runs
- Tradeoff: you're testing against recorded data, not a live system

This is actually the more practical approach for the assignment's 50+ test cases. You only need the live Docker stack for initial data exploration and fixture capture.

---

## Reference Material: Existing Agentic Implementations

Three prior projects provide reference patterns for building the agent. All three are Rust, but the architectural patterns transfer to any language.

### Abbot (Good — complex multi-agent system)

| Aspect        | Detail                                                                                                                                                                      |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Language      | Rust                                                                                                                                                                        |
| LLM providers | Anthropic + OpenAI (hand-rolled HTTP clients, no SDK)                                                                                                                       |
| Architecture  | Microkernel with Frame-based message bus; three agent roles (Head/Hand/Mind) running in parallel Rooms                                                                      |
| Tool pattern  | Co-located `.json` schema files next to `.rs` implementations; `tool__<ns>_<verb>` naming; catalog functions return `Vec<ToolSpec>` per role                                |
| Memory        | Per-agent private message history + shared Room transcript; SQLite persistence (3 databases: store, frames, EMS)                                                            |
| Observability | Every kernel Frame persisted to SQLite audit log; WebSocket frame stream; `KERNEL_TAP_FRAMES` env var for debug logging; admin REST endpoints                               |
| Testing       | Three-layer strategy: (1) Door trait mock for runner tests, (2) real kernel services for syscall tests, (3) unit tests. `MockDoor` + `UnifiedMockLlm` for integration tests |

**Relevant patterns for this project:**

- **Co-located JSON tool schemas** — tool definition (JSON) lives next to tool implementation. Clean separation of LLM interface from execution logic.
- **Tool catalog per role** — different agents get different tool subsets via catalog functions. Prevents tool sprawl.
- **Door trait as test boundary** — a single trait interface between agent orchestration and all external effects. Mock it for testing without LLM or database.
- **Frame audit log** — every operation persisted to SQLite. Provides full traceability for observability requirements.

**Overkill for this project:** Multi-agent rooms, parallel round execution, actor permission system, Mind proactive loop, safe mode circuit breaker, lane-based dispatcher concurrency.

### Prior (Good to Very Good — microkernel runtime)

| Aspect        | Detail                                                                                                                                                                      |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Language      | Rust                                                                                                                                                                        |
| LLM providers | Anthropic + OpenAI (hand-rolled HTTP clients)                                                                                                                               |
| Architecture  | Microkernel with subsystem registration; every subsystem communicates via universal Frame messages over typed channels                                                      |
| Tool pattern  | **Single `syscall` tool** — one open-schema tool covers all operations. LLM calls `syscall(syscall="vfs:read", data={path: "..."})` instead of `tool__fs_read(path: "...")` |
| Memory        | In-memory per-room history + FrameDb (SQLite) for persistence across restarts + EMS entity store for long-term memory                                                       |
| Observability | All frames persisted to SQLite; broadcast frames for LLM thinking/tool-use/chat; `PRIOR_TRACE_FRAMES` env var; CLI `prior tail` for live streaming                          |
| Testing       | Hygiene tests (source-scanning ratchet budgets), integration tests with mock subsystems, stress tests with configurable parameters                                          |

**Relevant patterns for this project:**

- **`LlmChat` trait for testability** — abstract the LLM call behind a trait; swap in `MockLlm` with pre-programmed response queues for deterministic testing. This is the most directly reusable pattern.
- **10-slot system prompt bundle** — fixed semantic slots (Identity, Commandments, Context, Tools, Description, Notes, Environment, Memory, Tone) with stable ordering for prompt caching. Clean way to compose complex system prompts.
- **Frame-based observability** — every operation is a Frame with id, parent_id, syscall name, status, and data. Makes tracing trivial.
- **Ratchet budget tests** — source-scanning tests that enforce code hygiene with numeric ceilings that can only shrink. Novel testing pattern.

**Overkill for this project:** Microkernel routing, distributed bridge, per-room workers, recursive delegation, pipe abstraction, VFS sandboxing.

### Gauntlet Week 1 (Meh — pragmatic single-agent)

| Aspect        | Detail                                                                                                                                               |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Language      | Rust + WASM (Leptos frontend)                                                                                                                        |
| LLM providers | Anthropic + OpenAI (hand-rolled HTTP clients)                                                                                                        |
| Architecture  | Single agent, iterative tool-call loop (max 10 rounds), one user prompt at a time                                                                    |
| Tool pattern  | `Vec<Tool>` returned from a registry function; tools are inline `serde_json::json!()` schemas; dispatch via match statement to `execute_*` functions |
| Memory        | In-memory session messages (3 messages, 600 chars each, 3,000 total cap); not database-persisted                                                     |
| Observability | Frame-based tracing with trace_id/span_id/parent_span_id/elapsed_ms; waterfall view in client                                                        |
| Testing       | `MockLlm` with pre-programmed response queues; thorough tool-level unit tests; no eval framework                                                     |

**Relevant patterns for this project (most directly applicable due to similar scope):**

- **Simple tool loop** — iterate up to N rounds: send to LLM, if tool_use then execute tools and feed results back, break when LLM stops calling tools. This is exactly the loop you need.
- **`LlmChat` trait + `MockLlm`** — same pattern as Prior but simpler. Pre-programmed response queues for testing multi-turn tool sequences without real API calls.
- **Tool inputs as `serde_json::Value`** — no typed input structs. Execute functions do manual field extraction. Fast to implement, easy to understand.
- **System prompt with board/context injection** — system prompt includes current state snapshot. Analogous to injecting patient context into the healthcare agent's system prompt.

**Weaknesses to avoid:** Monolithic `ai.rs` (2,600 lines with all tool implementations); no separation between orchestration and tool execution; no eval framework; session memory too small and not persisted.

### Cross-Reference: What to Reuse for OpenEMR Agent

| Need                        | Best reference           | Pattern                                                                                     |
| --------------------------- | ------------------------ | ------------------------------------------------------------------------------------------- |
| Agent tool loop             | Week 1                   | Simple iterate-until-done loop with max rounds                                              |
| Tool definitions            | Abbot                    | Co-located JSON schemas, catalog function returns tool list                                 |
| Tool dispatch               | Week 1                   | Match statement routing to execute functions (keep it simple)                               |
| LLM abstraction for testing | Prior / Week 1           | `LlmChat` trait + `MockLlm` with response queues                                            |
| System prompt composition   | Prior                    | Slot-based prompt bundle (identity, context, tools, memory)                                 |
| Observability/tracing       | Prior / Abbot            | Frame-based audit log; every operation gets an ID and parent_id                             |
| Conversation memory         | Prior                    | In-memory + SQLite persistence; reload on reconnect                                         |
| Eval framework              | ai-trials / faber-trials | YAML task definitions, layered graders, JSONL + SQLite dual-write, LLM-as-judge (see below) |

### Eval Framework References: ai-trials and faber-trials

Two additional projects provide **directly reusable eval patterns**. Both are purpose-built eval harnesses with mature architectures.

#### ai-trials (Python — most directly applicable)

| Aspect        | Detail                                                                                                                                     |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Language      | Python 3.11+, Click CLI, Rich terminal output                                                                                              |
| LLM providers | OpenAI + Anthropic + OpenRouter + CLI tools (claude, codex)                                                                                |
| Task format   | YAML files in `trials/NNN-group/*.yml` with `id`, `type`, `prompt`, `expected`, `tags`, `judge_criteria`                                   |
| Task types    | `exact` (string match), `contains` (substring/regex AND), `open` (LLM-judged)                                                              |
| Grading       | Three graders: ExactGrader (case-insensitive), ContainsGrader (multi-needle AND), JudgeGrader (LLM-as-judge with per-criterion 1-5 scales) |
| Results       | Dual-write: JSONL (crash-safe streaming) + SQLite (cross-run queries with pre-built aggregate views)                                       |
| Cost tracking | Per-trial and per-judge-call cost computed from model pricing config                                                                       |
| Filtering     | `--task "pattern*"` glob + `--tags simple,planning` multi-tag filter                                                                       |

**Key reusable patterns:**

- **YAML task definitions with inline judge criteria** — each criterion has `id`, `description`, `scale: [1, 5]` with anchors like `(1=blindly deletes, 5=confirms scope first)` baked into the description
- **TrialResult as complete audit record** — single dataclass captures prompt, response, tokens, latency, cost, grade, judge scores, judge model, judge cost. Nothing is lost.
- **Dual-write JSONL + SQLite** — JSONL for crash-safe streaming during runs; SQLite with pre-built views (`v_model_stats`, `v_task_stats`, `v_run_summary`) for cross-run analysis
- **Tag-based task organization** — tasks carry `tags: [tier, domain]` for flexible subset execution
- **Verifier pipeline stub** — TrialResult already has `verifier_model`, `verifier_response`, `transition` fields for a future generate → verify → judge pipeline

#### faber-trials (TypeScript/Bun — more sophisticated)

| Aspect           | Detail                                                                                                               |
| ---------------- | -------------------------------------------------------------------------------------------------------------------- |
| Language         | TypeScript on Bun runtime                                                                                            |
| LLM providers    | OpenAI SDK pointed at OpenRouter                                                                                     |
| Task format      | YAML files with `id`, `type`, `goal`, `input`, `expected_output`, `verdict_criteria`                                 |
| Task types       | Compilation-graded (deterministic: typecheck → run → output match) + LLM-judged (chain type with structured rubrics) |
| Grading          | Three-level deterministic (A=typecheck, B=runs, C=correct output) + LLM-as-judge for open-ended                      |
| Sweep dimensions | `models × n_shots × dialects × contexts × tasks` — 1,900 trials per model                                            |
| Pipeline mode    | Drafter → Verifier with transition matrix: preserved / damaged / recovered / failed                                  |
| Results          | JSONL + SQLite + AI-generated Markdown analysis (claude-3-haiku narrative)                                           |
| Versioning       | `framework_version` + `git_sha` stamped on every record                                                              |

**Key reusable patterns:**

- **Three-level grading** — instead of binary pass/fail, grade at multiple levels. For agents: A=tool selection correct, B=parameters correct, C=response accurate. Partial credit gives richer signal.
- **Transition matrix** — `preserved/damaged/recovered/failed` compares two stages. Directly reusable for evaluating agent self-correction or verification layers.
- **Multi-dimensional sweep** — parameterize over `(models × prompt_variants × context_levels × tasks)`. For agent evals: sweep over system prompt variants, tool documentation levels, patient complexity.
- **Response cleaning before grading** — strip markdown fences, extract structured data from conversational responses. Essential for agent evals where models add caveats and explanations.
- **Control group tasks** — pair known-good baseline tasks with experimental tasks to isolate the variable under test.
- **Framework versioning** — version stamp on every record prevents comparing incompatible runs when eval methodology changes.

#### What this means for the time estimate

The eval framework is **no longer a blank canvas**. Both ai-trials and faber-trials provide:

- Proven YAML task definition format
- Working grader implementations (exact, contains, LLM-as-judge)
- JSONL + SQLite dual-write pattern
- CLI runner with filtering and progress reporting
- Cost tracking infrastructure

The agent-specific adaptations needed:

- Task definitions that test tool selection, parameter correctness, and response synthesis
- A grader that validates tool call sequences (deterministic) alongside response quality (LLM-judged)
- Fixture data strategy (recorded API responses or mock data)
- Agent-specific judge criteria (hallucination detection, verification accuracy, safety refusal)

---

## Local Development: How Easy Is It to Spin Up?

### Steps to a working local fork

```bash
# 1. Fork and clone (standard GitHub flow)
git clone https://github.com/YOUR_USER/openemr.git
cd openemr

# 2. Start the dev stack
cd docker/development-easy
docker compose up --detach --wait

# 3. Wait 5-10 minutes on first boot (subsequent boots: < 1 minute)
# Container auto-runs: composer install, npm install, npm run build,
# database creation, schema install, migrations, Apache config

# 4. Access the app
# HTTP:  http://localhost:8300/
# HTTPS: https://localhost:9300/ (self-signed cert)
# Login: admin / pass

# 5. (Optional) Load demo data with clinical records
docker compose exec openemr /root/devtools dev-reset-install-demodata
docker compose exec openemr /root/devtools import-random-patients 20
```

**That's it.** No manual installer wizard, no config file editing, no database creation. The `flex` Docker image auto-installs everything.

### What you get after boot

| Service         | URL                            | Purpose                              |
| --------------- | ------------------------------ | ------------------------------------ |
| OpenEMR (HTTP)  | http://localhost:8300          | Main app                             |
| OpenEMR (HTTPS) | https://localhost:9300         | API endpoint (OAuth2 requires HTTPS) |
| phpMyAdmin      | http://localhost:8310          | Database browser                     |
| Swagger UI      | https://localhost:9300/swagger | API documentation                    |
| Mailpit         | http://localhost:8025          | Email capture UI                     |
| Selenium VNC    | http://localhost:7900          | E2E test browser viewer              |

### Local development gotchas

| Gotcha                                                                | Impact                                                                                                                                                |
| --------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| **5-10 minute cold start**                                            | First `docker compose up` downloads images + installs all deps + builds assets + initializes DB. Subsequent starts reuse volumes and take < 1 minute. |
| **`vendor/` and `node_modules/` live in Docker volumes, not on host** | IDE autocompletion for PHP/JS dependencies won't work unless you also run `composer install` and `npm install` on the host.                           |
| **CouchDB must be manually restarted after DB resets**                | Any `devtools dev-reset*` command breaks CouchDB state. Run `docker compose restart couchdb` afterward.                                               |
| **Xdebug is ON by default**                                           | Performance impact on every PHP request. Disable with `XDEBUG_ON: 0` in compose if not debugging PHP.                                                 |
| **7 containers running**                                              | Uses ~2-4GB RAM total. On a 16GB machine this is fine; on 8GB you'll feel it.                                                                         |
| **Self-signed TLS cert**                                              | Your Python agent's HTTP client must either disable TLS verification or trust the cert when calling `https://localhost:9300`.                         |
| **GitHub Composer token hardcoded in compose file**                   | Works out of the box but is a security anti-pattern. Don't push this to a public fork without removing it.                                            |

### For agent development specifically

You don't need the full 7-service stack for agent development. A **minimal stack** would be:

| Service    | Required?    | Why                                             |
| ---------- | ------------ | ----------------------------------------------- |
| mysql      | Yes          | Database                                        |
| openemr    | Yes          | API server                                      |
| phpmyadmin | Nice to have | Inspect data, debug queries                     |
| selenium   | No           | Only for E2E tests                              |
| couchdb    | No           | Document storage; not needed for API-only usage |
| openldap   | No           | LDAP auth testing                               |
| mailpit    | No           | Email testing                                   |

You could create a stripped `docker-compose.agent-dev.yml` with just mysql + openemr + phpmyadmin to reduce resource usage.

---

## Production Deployment: How Hard Is It?

### The production Docker stack

OpenEMR provides a **production-ready compose file** at `docker/production/docker-compose.yml` with only **2 services**:

```yaml
services:
  mysql:
    image: mariadb:11.8
    volumes:
      - databasevolume:/var/lib/mysql

  openemr:
    image: openemr/openemr:7.0.4 # Pinned release, pre-built
    ports:
      - 80:80
      - 443:443
    volumes:
      - sitevolume:/var/www/localhost/htdocs/openemr/sites
      - logvolume:/var/log
```

No Selenium, no phpMyAdmin, no CouchDB, no LDAP, no Mailpit. The production image has all assets pre-built — no `npm install` or `composer install` at boot. Still takes 5-10 minutes on first boot for DB initialization.

### Adding the Python agent to the stack

Extend the production compose with a third service:

```yaml
agent:
  build: ./agent
  ports:
    - 3000:3000 # Agent UI
  environment:
    OPENEMR_BASE_URL: http://openemr # Internal Docker network
    OPENEMR_API_USER: admin
    OPENEMR_API_PASS: pass
  depends_on:
    openemr:
      condition: service_healthy
```

The agent talks to OpenEMR over the internal Docker network (no external hop). Total: **3 services** for a complete demo deployment.

### Deployment platform analysis

| Platform                                | Viability  | Cost     | Notes                                                                                                           |
| --------------------------------------- | ---------- | -------- | --------------------------------------------------------------------------------------------------------------- |
| **VPS (DigitalOcean, Linode, Hetzner)** | Best       | $6-12/mo | Run `docker compose` directly. 2GB RAM, 50GB disk. Simplest path.                                               |
| **Fly.io**                              | Moderate   | $5-10/mo | Needs 2 Fly apps (OpenEMR + MariaDB) + persistent volumes. Must set `grace_period: 300s` for the 5-10 min boot. |
| **Railway**                             | Moderate   | $5/mo    | MariaDB add-on available. Docker deploy works. Health check timeouts configurable.                              |
| **Render**                              | Hard       | $7+/mo   | No managed MariaDB. Need two Docker services. No persistent disks on free tier.                                 |
| **Vercel / Netlify**                    | Impossible | —        | Not a containerized app. Cannot run PHP + MariaDB.                                                              |
| **AWS ECS / GCP Cloud Run**             | Overkill   | $15+/mo  | Works but complex for a demo. Cloud Run's cold-start timeout will fight the 5-10 min boot.                      |

### The deployment red flags

| Flag                                          | Severity | Detail                                                                                                                                                                                      |
| --------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **5-10 minute cold boot**                     | High     | First-boot DB initialization blocks all health checks. Platforms that kill containers after 30-60s of failed health checks will never let OpenEMR start. Must configure extended timeouts.  |
| **No Let's Encrypt integration**              | Medium   | Production image generates self-signed certs. For real HTTPS, you need a reverse proxy (Caddy, nginx, Traefik) in front doing TLS termination. Adds a 4th service or platform-level config. |
| **Two persistent volumes required**           | Medium   | `databasevolume` (MariaDB data) and `sitevolume` (site config, documents) must survive container restarts. Platforms without persistent disks are disqualified.                             |
| **MariaDB-only**                              | Medium   | No PostgreSQL support. Eliminates platforms that only offer managed Postgres (e.g., Render, Supabase).                                                                                      |
| **Large Docker image**                        | Low      | The production image is ~2GB+. Initial pull is slow. Subsequent deploys reuse layers.                                                                                                       |
| **OAuth2 base URL must match deployment URL** | Low      | The `OPENEMR_SETTING_site_addr_oath` env var must be set to your actual domain (e.g., `https://demo.example.com`). If you change domains, you must update this setting.                     |

### Minimum viable demo deployment recipe

1. Provision a $6-12/mo VPS (2GB RAM, 50GB SSD, Ubuntu 22.04)
2. Install Docker + Docker Compose
3. `docker compose -f docker-compose.prod.yml up -d` (OpenEMR + MariaDB + your agent)
4. Wait 5-10 minutes for first boot
5. Put Caddy in front for automatic HTTPS (Let's Encrypt)
6. Seed demo data: production image may not include devtools (`DEVELOPER_TOOLS` env var is not set in production compose). You may need to use the `flex` dev image instead, or seed data via the API, or pre-load SQL fixtures.
7. Point your domain at the VPS IP

**Total services in production: 4** (MariaDB + OpenEMR + Python agent + Caddy for TLS). Cost: ~$6-12/month.

### Comparison: what the assignment actually requires

The assignment says "Deployed and publicly accessible." This means:

- A URL someone can visit and interact with the agent
- The agent must be able to call OpenEMR's API
- Demo video shows it working

It does **not** require:

- Production-grade uptime
- Real patient data security
- HIPAA compliance
- Scalability

For a demo, the simplest path is a cheap VPS running docker compose with the 3-service stack (MariaDB + OpenEMR + agent). The 5-10 minute cold boot only happens once. After that, the stack stays up.

---

## Final Summary: Difficulty Assessment

### Overall difficulty: MODERATE-HIGH

This is a viable but friction-heavy choice. The core challenge is not complexity — it's indirection. Everything you need exists, but nothing is direct.

### Time budget estimate (7-day sprint)

| Work item                                        | Estimated effort | Why                                                                                                                                                         |
| ------------------------------------------------ | ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Fork, Docker up, explore APIs, seed data         | 3-4 hours        | Straightforward but slow cold start; need to figure out data seeding since production image may lack devtools                                               |
| OAuth2 client setup + token management in Python | 2-3 hours        | One-time boilerplate, but HTTPS-only + client registration + scope management is fiddly                                                                     |
| 5 agent tools (API wrappers)                     | 6-10 hours       | 2-3 tools are trivial API calls; drug interactions need external NLM API; availability needs custom logic                                                   |
| Agent orchestration (tool loop, prompt, memory)  | 4-6 hours        | Well-understood pattern from reference projects; Week 1 pattern transfers directly                                                                          |
| Frontend / chat UI                               | 3-5 hours        | Streamlit or simple React; independent of OpenEMR                                                                                                           |
| Eval framework (50+ test cases)                  | 5-8 hours        | Proven patterns from ai-trials and faber-trials (YAML tasks, layered graders, JSONL+SQLite); need agent-specific task definitions and fixture data strategy |
| Observability (tracing, latency, token tracking) | 3-5 hours        | LangSmith/Langfuse integration or custom logging                                                                                                            |
| Verification layer (3+ checks)                   | 3-4 hours        | Hallucination detection, confidence scoring, drug interaction severity validation                                                                           |
| Deployment to live demo                          | 3-5 hours        | VPS + docker compose + Caddy + data seeding + domain setup                                                                                                  |
| Documentation + demo video                       | 2-3 hours        | Architecture doc, cost analysis, recording                                                                                                                  |
| **Total**                                        | **~34-51 hours** | For a 7-day sprint, that's 5-7 hours/day                                                                                                                    |

### Where the time actually goes

The surprise is that **OpenEMR itself is not the hard part.** The API works, the data is there, and you don't touch PHP. The time sinks are:

1. **Eval framework (5-8 hours)** — Still a significant deliverable, but no longer a blank canvas. ai-trials provides a proven YAML task format, layered graders (exact/contains/LLM-as-judge), and JSONL+SQLite dual-write pattern. faber-trials adds three-level grading and transition matrices. You need agent-specific adaptations: tool call validation, fixture data strategy, and healthcare-specific judge criteria.

2. **Tool plumbing (6-10 hours)** — Not because the tools are conceptually hard, but because each one has a small annoyance: OAuth2 headers on every call, self-signed cert handling, exact-match-only search, raw status codes, missing joins requiring follow-up calls, external API for drug interactions.

3. **Deployment (3-5 hours)** — Three services + TLS proxy. The 5-10 minute cold boot and data seeding on production are the gotchas.

### What makes this easier than it looks

- **Zero PHP code to write or understand.** The 331K-line codebase is irrelevant to your implementation.
- **Clean API surface.** REST + FHIR with Swagger docs. Response formats are consistent and well-structured.
- **Medical coding baked in.** RxNorm, ICD-10, SNOMED codes come back inline with API responses. Makes verification meaningful without external lookups.
- **Docker dev environment is turnkey.** One command to a working system with login.
- **Domain prestige.** Healthcare EMR + FHIR compliance is impressive in a demo. The verification story writes itself.

### What makes this harder than it looks

- **The language gap is real.** PHP backend, Python/Node agent. Two ecosystems, two deployment targets, no code sharing. Every interaction is an HTTP round-trip.
- **No clinical demo data.** You will spend time either writing SQL fixtures, seeding via API, or wrestling with Synthea's randomness.
- **OAuth2 is mandatory overhead.** There's no shortcut. Every API call needs a bearer token. Token refresh must be handled.
- **The most impressive tools are the hardest.** Drug interaction checking (the demo showstopper) requires an external API. Appointment availability requires custom logic. Insurance verification is essentially impossible at the "real" level.
- **The eval framework needs agent-specific adaptation.** ai-trials and faber-trials provide the harness patterns, but you still need to design tool-call validation, healthcare-specific judge criteria, and a fixture data strategy.

### Risk matrix

| Risk                                         | Likelihood | Impact | Mitigation                                                                                                                |
| -------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------------------------------------------------- |
| Docker setup takes longer than expected      | Low        | Low    | Well-documented; fallback to `development-easy-light`                                                                     |
| OAuth2 integration bugs eat hours            | Medium     | Medium | Use the password grant (pre-enabled in dev); automate token refresh early                                                 |
| Drug interaction external API is flaky/slow  | Medium     | High   | Cache RxNav responses; have a fallback mock for demos                                                                     |
| Can't get deterministic eval data            | High       | Medium | Record API responses as fixtures; run evals against mocks                                                                 |
| Deployment platform fights the 5-10 min boot | Medium     | Medium | Use a plain VPS, not a PaaS; keep the stack running                                                                       |
| Run out of time on eval framework            | Low-Medium | High   | Proven patterns from ai-trials/faber-trials reduce design time; start evals on day 2; build incrementally alongside tools |

---

## Key Question for Decision-Making

The fundamental tension: OpenEMR is a **PHP system** and the assignment recommends **Python/Node agent frameworks**. This means:

- Your agent is a **separate service** calling OpenEMR's APIs
- You're deploying and maintaining **three services** (MariaDB + OpenEMR + your agent), plus a TLS proxy for production
- Your open source contribution either requires **PHP skills** (to contribute to OpenEMR itself) or lives in a **separate repository** (the Python agent)
- The complexity budget spent on OAuth2 setup, Docker orchestration, and API plumbing is time **not** spent on agent logic, evals, and observability

Compare this against the alternative: if the other source repo is in a language compatible with the recommended agent frameworks, or has a simpler API surface, or has more of the 5 tools ready out-of-the-box, that reduces integration friction and lets you focus on the actual agent deliverables.
