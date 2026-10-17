---
date: 2026-10-16
questions:
  - How can a Windows desktop utility resolve an address or city into saved coordinates and compute an independent sunrise and sunset schedule?
  - Do free solar APIs or refreshes near sunrise and sunset improve the scheduling options compared with local calculation?
  - What would a Google geocoding integration cost, and which credential and storage conditions affect it?
observed-in:
  - Sunrise-Sunset.org API v2 documentation retrieved 2026-10-16
  - SunriseSunset.io API documentation retrieved 2026-10-16
  - Nominatim public usage policy and 5.3.3 search documentation retrieved 2026-10-16
  - Photon repository documentation retrieved 2026-10-16
  - Geoapify pricing and geocoding documentation retrieved 2026-10-16
  - Google global pricing, Geocoding API policies, and security guidance retrieved 2026-10-16
  - Google Maps Platform non-EEA Service Specific Terms last modified 2026-10-15
  - sunrise 3.0.0 API documentation retrieved 2026-10-16
  - USNO, NOAA, and Microsoft documentation retrieved 2026-10-16
recheck-when: A selected provider changes its pricing or usage policy, the solar algorithm changes, or deployment introduces a different location-permission model.
---

# Location lookup and independent solar scheduling

Local solar calculation is a viable way to schedule a Windows Night Light utility after it saves the user's coordinates. Address or city lookup can remain an occasional network operation in settings, with Windows location as an optional alternative. Free solar APIs also support advance date ranges, but repeatedly requesting the same coordinates and date near sunrise does not ordinarily improve a calculation with fixed astronomical assumptions. The recommended design is submitted location search, explicit result selection, saved coordinates, and local solar calculation; geocoding-provider selection and polar-day behavior are separate product choices. Google includes 10,000 geocoding requests per month without an API charge, but its credential deployment and coordinate-storage conditions also affect provider selection.

## Decision context

Solar scheduling and location acquisition are post-MVP work under [SPEC.md](../specs/SPEC.md#future-work). This document compares location providers and solar-service alternatives for that work. The [local solar validation](solar-calculation.md) records measured library behavior, including polar and date-boundary cases. These findings do not add location inputs or solar dependencies to the Wake/Bedtime MVP.

The utility turns off the Windows Night Light schedule and computes its own screen-temperature schedule. An address-search feature would need to convert an entered address or city into latitude and longitude. Scheduling needs to retain coordinates independently of Windows Night Light and determine solar events on future days. The choice is whether those events require an online service and, separately, which location-search service supports the intended settings experience.

This document records documentation and source inspection. It does not report live API responses, compiled Rust behavior, measured calculation error, or Windows permission tests. Recommendations are proposals rather than additional requirements.

## Solar times describe a calculation convention

**Observed in United States Naval Observatory (USNO) definitions retrieved 2026-10-16:** Standard sunrise and sunset calculations place the Sun's center 50 arcminutes below a level horizon: 16 arcminutes for the apparent solar radius and 34 for average atmospheric refraction. This corresponds to the upper edge meeting an unobstructed horizon at sea level under average conditions. Civil twilight instead uses the solar center six degrees below the horizon. These are different events. [USNO rise, set, and twilight definitions](https://aa.usno.navy.mil/faq/RST_defs).

**Observed in National Oceanic and Atmospheric Administration (NOAA) calculation documentation retrieved 2026-10-16:** NOAA describes a calculation based on Jean Meeus's astronomical algorithms and publishes theoretical sunrise/sunset accuracy of one minute within latitude ±72 degrees and ten minutes beyond that range. It also says atmospheric variation can change observed times and that its calculator is no longer maintained. Those statements describe NOAA's calculator; they do not establish the accuracy of another implementation. [NOAA calculation details](https://gml.noaa.gov/grad/solcalc/calcdetails.html).

**Inference:** With the date, coordinates, elevation, event definition, algorithm, and relevant time rules fixed, calculating an event again closer to its occurrence supplies no new physical observation. An actual visible sunrise can differ because of atmospheric refraction and the local horizon. A conventional solar API is not a weather or terrain-obstruction observation service merely because its timestamps include seconds. Refreshing can recover missing data or adopt changed inputs or algorithms; proximity to the event alone is not an accuracy benefit.

## Solar calculation alternatives

**Observed in the provider and library documentation retrieved 2026-10-16:**

| Option | Cost and access | Date and event behavior | Operational tradeoff |
| :-- | :-- | :-- | :-- |
| Local Rust calculation | No solar service or API key; library license applies. | The `sunrise` 3.0.0 API accepts coordinates and a date, optionally elevation, and returns UTC event times or `None` for an absent event. | Scheduling can run without a network connection; the separate reference comparison rejects this candidate's polar behavior. |
| Sunrise-Sunset.org v2 | Free, keyless; visible attribution required; reasonable-volume policy without a numerical quota. HTTP 429 includes `Retry-After`. | Up to 366 dates per request; ISO 8601 or Unix output; detected or explicit IANA time zone; independently nullable events and a solar-state field. | An online dependency with explicit cache support. [API documentation](https://sunrise-sunset.org/api). |
| SunriseSunset.io | Free for personal and commercial use, keyless; backlink required; reasonable-use policy without a fixed enforced quota. | Up to 365 dates per request; ISO 8601 or Unix output; time-zone detection; explicit polar-state field and nullable events. | Terrain elevation is included by default and can be overridden or disabled. [API documentation](https://sunrisesunset.io/api/). |

**Observed in Sunrise-Sunset.org v2 documentation retrieved 2026-10-16:** Explicit-date responses are documented as immutable, and the provider recommends caching a year rather than requesting each day. The calculation uses a solar-center altitude of −0.833 degrees for sunrise and sunset. Its v1 calculation differs from v2; an API version is part of the calculation choice. A polar transition date can have only one event even when its solar-state field is `normal`. [API documentation](https://sunrise-sunset.org/api).

**Observed in SunriseSunset.io documentation retrieved 2026-10-16:** Default elevation comes from the ETOPO1 terrain-elevation dataset; `elevation=false` selects sea level. Its default text times omit the date, so machine consumers can request ISO 8601 or Unix timestamps. An invalid explicit time zone can return HTTP 200 with `INVALID_TZID` and a fallback zone. [API documentation](https://sunrisesunset.io/api/).

**Inference:** An API cache should distinguish coordinates, requested dates, time-zone interpretation, elevation policy, and calculation version. Several cached days are sufficient to reduce network calls; a larger range extends operation through an outage. An exhausted cache still needs a defined failure policy. Local calculation avoids this cache-coverage dependency. Comparisons between implementations must use the same elevation and event convention before interpreting differences as errors.

## Candidate API and separate validation

**Observed in `sunrise` 3.0.0 documentation and repository material:** `SolarDay::event_time` returns `Option<DateTime<Utc>>`; the documented absent-event example is sunset during a polar day. `with_altitude` defaults to zero meters. The project is MIT licensed and supports calculation without the Rust standard library when configured with `libm`. These APIs demonstrate a local implementation path. [Versioned API documentation](https://docs.rs/sunrise/3.0.0/sunrise/struct.SolarDay.html), [project repository](https://github.com/nathan-osman/rust-sunrise).

The crate's published documentation does not establish an error tolerance for this app. The separate [reference comparison](solar-calculation.md) tests its date-line and polar behavior and rejects `sunrise` 3.0.0 because it misses polar events. That research selects `solar-positioning` 0.7.1; the API description above remains evidence about the evaluated alternative.

## Location-search alternatives

Geocoding means converting a place name or address into geographic coordinates. It is separate from solar calculation: saving a selected result means that daily scheduling need not repeat the search.

**Observed in documentation and policies retrieved 2026-10-16:**

| Provider | Free access and limits | Search and saved-result implications |
| :-- | :-- | :-- |
| Public Nominatim | Keyless public service; maximum one request per second across the entire application's users. | Free-form and structured address search return candidate coordinates. Public-endpoint restrictions below affect the UI and distribution. [Search API](https://nominatim.org/release-docs/latest/api/Search/), [usage policy](https://operations.osmfoundation.org/policies/nominatim/). |
| Photon public demo | Public endpoint; reasonable usage permitted without a stated numerical allowance. Heavy use can be throttled or banned. | Photon supports search-as-you-type. The demo has no availability guarantee and can change without notice. Its OpenStreetMap basis still requires attention to data attribution. [Project documentation](https://github.com/komoot/photon). |
| Geoapify | Free plan: 3,000 credits/day and up to five requests/second; account and API key required; no credit card required. A normal geocoding request costs one credit. | Supports addresses and cities; saved results are permitted with attribution retained. Free commercial production use is permitted within plan limits. [Pricing](https://www.geoapify.com/pricing/), [geocoding documentation](https://www.geoapify.com/geocoding-api/). |

**Observed in the public Nominatim policy retrieved 2026-10-16:** User-submitted searches are permitted for moderate usage. The application must identify itself through its HTTP User-Agent or Referer, display attribution, and permit switching services without a software update. Caching is encouraged; repeated bulk queries must be cached. Client-side autocomplete is forbidden, including a search request after every typing pause. Periodic geocoding is discouraged. The policy also prohibits submitting personal or confidential material and requires an informed developer decision for an AI-assisted integration. These restrictions apply to the public endpoint, not automatically to other Nominatim hosts. [Nominatim usage policy](https://operations.osmfoundation.org/policies/nominatim/).

**Observed in Geoapify developer documentation retrieved 2026-10-16:** Results can include a display address, latitude/longitude, match-confidence fields, and time-zone metadata. Some fields can be absent. [Response fields](https://apidocs.geoapify.com/docs/geocoding/forward-geocoding/).

**Recommendation:** Use an explicit Search action and a candidate list that includes locality and country. Save the selected latitude/longitude and a useful display label. Preserve any required provider attribution with saved data. Keep the provider decision open: public Nominatim needs deliberate acceptance of its restrictions; Photon offers a keyless option with weaker service guarantees; Geoapify offers explicit quotas and storage terms but introduces an account/key decision. An unrestricted personal-address field should not silently use a public service whose policy excludes personal data.

## Google geocoding cost and deployment

**Observed in Google's global pricing and usage documentation retrieved 2026-10-16:** Geocoding includes 10,000 requests per month at no charge, then costs USD 5 per 1,000 through 100,000 monthly requests. Usage is aggregated across projects on the billing account. Billing and credentials are required even within the free allowance. Daily quotas can stop requests at a configured limit. [Pricing](https://developers.google.com/maps/billing-and-pricing/pricing), [usage and billing](https://developers.google.com/maps/documentation/geocoding/usage-and-billing).

**Calculated from those published rates:**

| Monthly geocoding requests across users | Monthly API charge, USD |
| :-- | --: |
| 1,000 | 0 |
| 10,000 | 0 |
| 20,000 | 50 |
| 50,000 | 200 |
| 100,000 | 450 |

These estimates exclude taxes, hosting, map display, and autocomplete. They count requests, including repeated searches, rather than installations. Local solar calculations do not call Google's geocoder.

**Observed in Google's Maps JavaScript billing documentation retrieved 2026-10-16:** Interactive map display and geocoding use separate billing categories. A typed Search action can resolve coordinates without displaying a map. [JavaScript billing](https://developers.google.com/maps/documentation/javascript/usage-and-billing), [Geocoding service terms, section 6.1](https://cloud.google.com/maps-platform/terms/maps-service-terms).

**Observed in Google's security guidance retrieved 2026-10-16:** Browser keys support website restrictions. Web-service keys are intended to remain secret; Google recommends an authenticated proxy for client applications without a suitable client SDK. [Security guidance](https://developers.google.com/maps/api-security-best-practices).

**Design inference:** A Circadia-owned HTTPS lookup page can use a restricted browser key and return coordinates for entry into the app. Native settings can instead call a small hosted proxy with abuse controls. Both approaches add deployment work; embedding a shared secret in the distributed executable does not protect it. Hosting cost remains unpriced until the deployment is selected. Neither approach has been implemented or tested with Circadia-owned credentials.

**Observed in the Service Specific Terms for accounts outside the European Economic Area (EEA) last modified 2026-10-15:** Section 6.3.1 permits temporary coordinate caching for 30 days. Section 6.3.2 additionally permits indefinite storage of coordinates and address fields for direct user-facing functionality in the requesting application, subject to these conditions:

- The stored data is isolated to its associated user.
- The data is not reused across users.
- Google restricts using cached data to replace additional service calls; how this condition applies to Circadia's offline schedule remains unverified.

Thus a blanket 30-day limit omits an explicit exception. The sources reviewed here do not resolve when an additional service call is required for Circadia's saved location and offline scheduling. That condition needs confirmation before selecting Google. These findings do not establish the conditions for an EEA billing account. [Service Specific Terms, section 6](https://cloud.google.com/maps-platform/terms/maps-service-terms).

**Observed in Geocoding API policies retrieved 2026-10-16:** Displayed results require Google Maps attribution, including when there is no map. [Attribution policy](https://developers.google.com/maps/documentation/geocoding/policies).

**Recommendation:** If Google is selected, use submitted geocoding searches without a map or autocomplete unless those features serve a separate UI requirement. Select the credential deployment and verify saved-coordinate use before committing to the provider. The free allowance can cover modest search traffic; it does not eliminate deployment or provider-policy responsibilities.

## Optional Windows location

**Observed in Microsoft's WinUI 3 location guide retrieved 2026-10-16:** `Geolocator.RequestAccessAsync` must run on the UI thread while the app is in the foreground. A one-time `GetGeopositionAsync` call can then obtain coordinates. Packaged apps declare the location capability; the current guide says unpackaged apps do not need that manifest declaration but still request access. Disabled services, denied access, and unavailable position data are normal outcomes. [Microsoft location guide](https://learn.microsoft.com/en-us/windows/apps/develop/maps-and-location/get-location).

**Recommendation:** Offer Use current location in settings, show the resulting coordinates, and save them through the same configuration path as a search result. Retain the previous saved location if detection or search fails. Automatic travel tracking and reverse-geocoding a detected position are separate features; neither is necessary for a schedule based on a saved location.

## Calendar days, time zones, and polar conditions

A timestamp is an instant; a calendar date depends on a time zone. The design needs an explicit rule for whether dates follow the configured location or the computer's Windows time zone. Use UTC instants for event ordering and the applicable date-specific zone rules for display. A numeric UTC offset saved during setup does not represent future daylight-saving transitions.

**Recommendation:** Retain surrounding dates when selecting the previous and next solar events, then evaluate the temperature schedule from the current instant. Recompute after a location change or a relevant time or time-zone change. A missing sunrise and a missing sunset must be treated independently. Missing events do not identify an API failure by themselves.

An absent event does not select a product fallback by itself. Continuous daylight could retain 6500 K; continuous night could retain a selected warm temperature. Alternatively, the app could offer fixed clock times. Neither choice follows automatically from an astronomical library's `None` result, and transition days with only one event require a separate acceptance scenario.

## Recommendation and remaining evidence

Prefer local solar calculation after an explicitly selected location. Keep online access in the settings flow for address/city search and optional Windows location. If an online solar service is preferred, retrieve explicit future dates in a batch and retain their results; do not add near-event refreshes solely to improve accuracy.

A future solar/location feature would need these decisions:

- Choose the geocoding provider and accept its attribution, query, and credential requirements.
- Choose the calendar-day/time-zone rule and the behavior when the saved location differs from the computer's location.
- Choose the solar event convention, elevation policy, and a measurable comparison tolerance.
- Choose continuous-daylight, continuous-night, and single-event-day behavior.
- If solar scheduling uses an API, choose cache coverage, retry timing, and behavior after cached dates are exhausted.

The separate [solar comparison](solar-calculation.md) validates the tested Rust calculation, including date-line boundaries and polar transitions. The [combined probe](schedule-integration.md) verifies Windows date-boundary integration, and the [Reactor research](windows-reactor-ui.md) records a successful location request on that desktop. The location code is preserved in the [source archive](archives/README.md); the active Reactor probe does not request location. Provider search coverage and additional permission outcomes remain unverified future-feature work.

## Sources

All unversioned pages below were retrieved on 2026-10-16. Provider policies and prices describe that date.

| Source | Evidence used |
| :-- | :-- |
| [USNO rise, set, and twilight definitions](https://aa.usno.navy.mil/faq/RST_defs) | Event convention and average atmospheric assumptions. |
| [NOAA solar calculation details](https://gml.noaa.gov/grad/solcalc/calcdetails.html) | Calculation method, scoped accuracy claim, physical variation, maintenance status. |
| [Sunrise-Sunset.org API v2](https://sunrise-sunset.org/api) | Free access, date ranges, caching, time zones, polar events, attribution. |
| [SunriseSunset.io API](https://sunrisesunset.io/api/) | Free access, date ranges, elevation, response semantics, attribution. |
| [`sunrise` 3.0.0 `SolarDay`](https://docs.rs/sunrise/3.0.0/sunrise/struct.SolarDay.html) | Local calculation inputs and absent-event representation. |
| [`rust-sunrise` repository](https://github.com/nathan-osman/rust-sunrise) | Library description, offline calculation example, license. |
| [Nominatim 5.3.3 search API](https://nominatim.org/release-docs/latest/api/Search/) | Address and city lookup. |
| [Public Nominatim usage policy](https://operations.osmfoundation.org/policies/nominatim/) | Public-endpoint limits and permitted usage. |
| [Photon repository](https://github.com/komoot/photon) | Search capabilities and demo-server limitations. |
| [Geoapify pricing](https://www.geoapify.com/pricing/) | Free quota and rate limit. |
| [Geoapify geocoding](https://www.geoapify.com/geocoding-api/) | Request cost, credentials, saved data, attribution. |
| [Geoapify response fields](https://apidocs.geoapify.com/docs/geocoding/forward-geocoding/) | Coordinates, labels, confidence, optional time-zone metadata. |
| [Microsoft desktop location guide](https://learn.microsoft.com/en-us/windows/apps/develop/maps-and-location/get-location) | Foreground permission flow and one-time position acquisition. |
| [Google global pricing](https://developers.google.com/maps/billing-and-pricing/pricing) | Monthly free allowance and request prices. |
| [Google Geocoding usage and billing](https://developers.google.com/maps/documentation/geocoding/usage-and-billing) | Billing, credentials, and quota controls. |
| [Google Maps JavaScript billing](https://developers.google.com/maps/documentation/javascript/usage-and-billing) | Separate map-display and geocoding billing categories. |
| [Google Maps API security guidance](https://developers.google.com/maps/api-security-best-practices) | Browser restrictions and secure proxy guidance. |
| [Google Maps non-EEA Service Specific Terms](https://cloud.google.com/maps-platform/terms/maps-service-terms) | Geocoding without a map and conditional indefinite per-user storage. |
| [Google Geocoding policies](https://developers.google.com/maps/documentation/geocoding/policies) | Attribution and billing-region distinction. |
