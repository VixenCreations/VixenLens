<script lang="ts">
  import type { Config } from '$lib/config'
  import { init, locale, register, waitLocale,t } from 'svelte-i18n'
  import { configStore } from '../../stores'
  import { onMount } from 'svelte'
  import { getConfig } from '$lib/api'
  import { resolveResource } from '@tauri-apps/api/path';
  import { readTextFile } from '@tauri-apps/plugin-fs';
  import Layout from '$lib/components/Layout.svelte'
  import { statusStore } from '../../statusStore'
  import { openUrl } from '@tauri-apps/plugin-opener'


  let config: Config
  let isLocaleReady = false
  let isConfigReady = false
  let statusMessage: string = '';
  let isVisible = false;
  let status = {
    message: '',
    progress: null as number | null,
    type: 'info',
    isVisible: true,
  }
  $: statusStore.subscribe((value) => (status = value))
  $: configStore.subscribe((value) => {
    config = value
  })

  let fossaContent: string;
  let icon;

  // i18n設定
  async function initializeI18n() {
    register('en', () => import('$lib/locale/about/en.json'))
    register('ja', () => import('$lib/locale/about/ja.json'))
    init({
      fallbackLocale: 'ja',
      initialLocale: 'en',
    })
    await waitLocale()
    isLocaleReady = true
  }

  onMount(async () => {
    await initializeI18n()
    config = await getConfig()
    isConfigReady = true
    locale.set(config.feature_flags.language)
    await waitLocale()
    fossaContent = await readTextFile(await resolveResource('resources/FOSSA_REPORT.txt'));
  })


  function openGithub() {
    openUrl("https://github.com/Vixenlicious");
  }
</script>

<Layout
  activePage="about"
  {statusMessage}
  thumbnailKeys={[]}
  thumbnailProcessed={false}
>
  {#if isLocaleReady && isConfigReady}
    <!-- アプリ概要 -->
     <div class="about-container">

    <h1 id="about">{$t('about')}</h1>

    <!-- アイコン表示 -->
    <div class="icon-container">
      <img src="/icon.png" alt="App Icon" width="100" height="100" />
    </div>
    <p>{$t('app_description')}</p>

    <hr />

    <!-- アプリ機能 -->
    <h2>{$t('features')}</h2>
    <ul>
      <li>{$t('feature_metadata_analysis')}</li>
      <li>{$t('feature_search')}</li>
      <li>{$t('feature_ui')}</li>
      <li>{$t('feature_folder_analysis')}</li>
      <li>{$t('feature_fast_safe')}</li>
    </ul>

    <hr />

		<!-- Changelog セクション（非表示中） -->
		<div class="changelog">
			<h2>{$t('changelog')}</h2>
			<p>{$t('changelog_placeholder')}</p>
		</div>

		<!-- アプリ情報 -->
		<h2>{$t('app_info')}</h2>

		<p>
			<strong>{$t('version')}:</strong> 1.0.3
		</p>

		<p>
			<strong>{$t('author')}:</strong> Vixenlicious
		</p>

		<p>
			<strong>{$t('contact')}:</strong>
			<button
				type="button"
				class="link-button"
				on:click={openGithub}
			>
				Github
			</button>
		</p>

    </div>
    <!-- /.about-container -->

    <!-- FOSSA ライセンス情報 -->
    <div class="license-container">
      <h2>{$t('licenses')}</h2>
				<button
					type="button"
					class="toggle-button"
					class:open={isVisible}
					on:click={() => {
						isVisible = !isVisible
					}}
					aria-expanded={isVisible}
					aria-label={$t('licenses_toggle_label')}
				></button>
    </div>

    {#if isVisible}
      <div class="fossa-container">
        <pre>{fossaContent}</pre>
        <!-- FOSSAファイル内容を埋め込み -->
      </div>
    {/if}
  {/if}
</Layout>

<style>
    .icon-container {
        display: flex;
        justify-content: center;
        margin-bottom: 20px;
    }

    .icon-container img {
        border-radius: 12px; /* アイコンに丸みを追加 */
    }

    h1, h2 {
        text-align: center;
    }

    ul {
        list-style-position: inside;
        padding-left: 0;
    }

    .changelog {
        display: none; /* Changelog セクションを非表示に設定 */
    }

	.fossa-container {
		height: 40vh;
		overflow: scroll;
		background-color: #ffffff;
		border: 1px solid #333;
		border-radius: 0.1rem;
		padding: 1rem;
	}

	.fossa-container pre {
		font-size: 0.7rem;
		white-space: pre-wrap;
	}

	.toggle-button {
		all: unset;
		display: block;
		margin: 5px;
		padding-top: 0.34rem;

		font-size: 0.8rem;
		line-height: 28px;
		text-align: center;
		color: #222;

		cursor: pointer;
		user-select: none;

		transition:
			transform 0.2s ease,
			background-color 0.3s ease;
	}

	.toggle-button:focus-visible {
		outline: 2px solid #007bff;
		outline-offset: 2px;
	}

	/* Arrow indicator (collapsed) */
	.toggle-button::before {
		content: '◀';
		transition: transform 0.2s ease;
	}

	/* Arrow indicator (expanded) */
	.toggle-button.open::before {
		content: '▼';
		transform: rotate(90deg);
	}

	.link-button {
		background: none;
		border: none;
		padding: 0;
		margin-left: 0.3rem;
		color: #007bff;
		font-weight: bold;
		cursor: pointer;
		text-decoration: none;
	}

	.link-button:hover {
		color: #0056b3;
		text-decoration: underline;
	}

	.link-button:focus-visible {
		outline: 2px solid #007bff;
		outline-offset: 2px;
	}



  .license-container{
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 1rem;
  }

  .about-container{
    padding: 0 20%;
  }

</style>