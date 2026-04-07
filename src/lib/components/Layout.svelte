<script context="module" lang="ts">
  import { init, register } from 'svelte-i18n'

  const g = globalThis as any

  if (!g.__VRCXPS_I18N_BOOTSTRAPPED__) {
    g.__VRCXPS_I18N_BOOTSTRAPPED__ = true

    register('en', () => import('$lib/locale/en.json'))
    register('ja', () => import('$lib/locale/ja.json'))

    // Must run before any $t/$tr formatting happens
    init({
      fallbackLocale: 'ja',
      initialLocale: 'en'
    })
  }
</script>

<script lang="ts">
  import { page } from '$app/stores'
  import { derived, get } from 'svelte/store'
  import { onMount } from 'svelte'
  import { locale, t, waitLocale } from 'svelte-i18n'

  import HomePane from './leftpane/HomePane.svelte'
  import SettingsPane from './leftpane/SettingsPane.svelte'

  import { configStore } from '../../stores'
  import { statusStore } from '../../statusStore'
  import { getConfig } from '$lib/api'
  import type { Config } from '$lib/config'

  // Props
  export let activePage: string
  export let thumbnailKeys: string[] = []
  export let thumbnailProcessed: boolean = false
  export let statusMessage: string = ''

	// Returns fallback when i18n returns the key (meaning missing translation)
	const tr = derived(t, ($t) => (key: string, fallback: string) => {
		try {
			const v = $t(key)
			return v === key ? fallback : v
		} catch {
			// If i18n isn't initialized yet (or HMR race), never crash the app.
			return fallback
		}
	})
	
  // Store-backed state (no manual subscribe)
  let config: Config | null
  $: config = $configStore ?? null

  let status: { message: string; progress: number | null; type: string; isVisible: boolean }
  $: status = $statusStore ?? { message: '', progress: null, type: 'info', isVisible: true }

  // Readiness signals
  $: isConfigReady = !!config

  // ---------------------------
  // i18n bootstrap (MUST run before first render)
  // ---------------------------
	let isLocaleReady = false

	onMount(async () => {
		await waitLocale()
		isLocaleReady = true
		await ensureConfigLoaded()
	})

  // Ensure config exists globally (avoid “blank until a page loads config”)
  async function ensureConfigLoaded() {
    if (get(configStore)) return
    const loaded = await getConfig()
    configStore.set(loaded)
  }

  // Apply config language once i18n is ready
	let appliedLang: string | null = null
	$: if (isLocaleReady && config?.feature_flags?.language && appliedLang !== config.feature_flags.language) {
		appliedLang = config.feature_flags.language
		locale.set(appliedLang)
	}

  // Left pane component per route
  const currentComponent = derived(page, ($page) => {
    switch ($page.route.id) {
      case '/':
        return HomePane
      case '/settings':
        return SettingsPane
      case '/about':
        return null
      default:
        return null
    }
  })

  // Split pane sizing
  let leftPaneWidth = 250
  let isResizing = false

  function clampLeftWidth(px: number) {
    const min = 250
    const max = 600
    return Math.max(min, Math.min(max, px))
  }

  function startResize(e: MouseEvent | KeyboardEvent) {
    // Keyboard “start” just toggles focus affordance; mouse does actual drag.
    if (e instanceof MouseEvent) {
      isResizing = true
      document.addEventListener('mousemove', handleResize)
      document.addEventListener('mouseup', stopResize)
    }
  }

  function handleResize(e: MouseEvent) {
    if (!isResizing) return
    leftPaneWidth = clampLeftWidth(e.clientX)
  }

  function stopResize() {
    isResizing = false
    document.removeEventListener('mousemove', handleResize)
    document.removeEventListener('mouseup', stopResize)
  }

  function nudgeResize(delta: number) {
    leftPaneWidth = clampLeftWidth(leftPaneWidth + delta)
  }
</script>

<div class="container">
  <header class="top-menu">
    <nav aria-label="Main">
      <ul>
        <li>
          <a href="/" class="menu-link" class:active={activePage === 'home'}>
            {$tr('layout.menu.home', 'Home')}
          </a>
        </li>
        <li>
          <a
            href="/settings"
            class="menu-link"
            class:active={activePage === 'settings'}
          >
            {$tr('layout.menu.settings', 'Settings')}
          </a>
        </li>
        <li>
          <a href="/about" class="menu-link" class:active={activePage === 'about'}>
            {$tr('layout.menu.about', 'About')}
          </a>
        </li>
      </ul>
    </nav>
  </header>

  <div class="main-content" style={`grid-template-columns: ${leftPaneWidth}px 5px 1fr;`}>
    <aside class="left-pane" aria-label="Left pane">
      {#if !isConfigReady}
        <p class="loading">{isLocaleReady ? 'Loading settings…' : 'Loading…'}</p>
      {:else if $currentComponent}
        <svelte:component this={$currentComponent} {thumbnailKeys} />
      {:else}
        <p>{$tr('layout.left_pane.no_content', 'No content')}</p>
      {/if}
    </aside>

		{#if isConfigReady}
			<button
				type="button"
				class="resizer"
				aria-label={$tr('layout.resizer', 'Resize panes')}
				on:mousedown={startResize}
				on:keydown={(e) => {
					if (e.key === 'ArrowLeft') nudgeResize(-10)
					if (e.key === 'ArrowRight') nudgeResize(10)
					if (e.key === 'Home') leftPaneWidth = 250
					if (e.key === 'End') leftPaneWidth = 600
					if (e.key === 'Enter' || e.key === ' ') startResize(e)
				}}
			></button>
		{:else}
			<div
				class="resizer disabled"
				aria-hidden="true"
			></div>
		{/if}

    <section class="right-pane" aria-label="Main content">
      {#if isConfigReady}
        <slot />
      {:else}
        <p class="loading">{isLocaleReady ? 'Loading…' : 'Loading…'}</p>
      {/if}
    </section>
  </div>

  <footer class={`status-bar ${status?.type ?? 'info'}`}>
		<p>
			{statusMessage ||
				status?.message ||
				$tr('layout.status_bar.default_message', 'Ready')}
			{#if thumbnailProcessed}
				<span class="status-pill">OK</span>
			{/if}
		</p>
  </footer>
</div>

<style>
  .container {
    display: grid;
    grid-template-rows: auto 1fr auto;
    height: 100vh;
    overflow: hidden;
  }

  .top-menu {
    grid-column: 1 / -1;
    background-color: #333;
    color: white;
    padding: 0.5rem 1rem;
  }

  .top-menu ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    gap: 1rem;
  }

  .menu-link {
    color: white;
    text-decoration: none;
    font-size: 16px;
  }

  .menu-link:hover {
    color: #00d4ff;
  }

  .menu-link.active {
    text-decoration: underline;
    text-underline-offset: 4px;
  }

  .main-content {
    display: grid;
    grid-template-columns: 250px 5px 1fr;
    overflow: hidden;
  }

  .left-pane {
    background: #eff0f0;
    border-right: 1px solid #ddd;
    padding: 1rem;
    overflow-y: auto;
    min-width: 150px;
    max-width: 600px;
  }

  .loading {
    margin: 0.5rem 0;
    color: #333;
    font-size: 14px;
    opacity: 0.8;
  }

	.resizer {
		all: unset;
		background-color: #ddd;
		cursor: ew-resize;
		width: 5px;
		user-select: none;
	}

	.resizer:hover {
		background-color: #bbb;
	}

	.resizer:focus-visible {
		outline: 2px solid #00d4ff;
		outline-offset: -2px;
	}

	.resizer.disabled {
		cursor: default;
		opacity: 0.4;
		pointer-events: none;
	}

  .right-pane {
    padding: 1rem;
    overflow-y: auto;
  }

  .status-bar {
    grid-row: 3;
    grid-column: 1 / -1;
    height: 2rem;
    background-color: #333;
    color: white;
    display: flex;
    align-items: center;
    padding: 0 1rem;
    font-size: 14px;
  }

  .status-bar p {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .status-bar.info {
    background-color: #333;
  }

  .status-bar.success {
    background-color: #4caf50;
    animation: revert-color 3s ease-in-out forwards;
  }

  .status-bar.error {
    background-color: #f44336;
    animation: revert-color 3s ease-in-out forwards;
  }

  .status-pill {
    display: inline-block;
    padding: 0.1rem 0.4rem;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.2);
    font-size: 12px;
  }

  @keyframes revert-color {
    100% {
      background-color: #333;
    }
  }
</style>
