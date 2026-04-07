<script lang="ts">
  import Layout from '../lib/components/Layout.svelte'
  import { onMount, onDestroy, afterUpdate } from 'svelte'
  import {
    getAllFolders,
    getConfig,
    getInitialSetupState,
    getMetadata,
    getThumbnailsChunk,
  } from '$lib/api'
	import { searchImage } from '$lib/api'
  import { goto } from '$app/navigation'
  import { configStore, thumbnailStore, updateDBWhenInit } from '../stores'
  import type { Config } from '$lib/config'
  import flatpickr from 'flatpickr' // カレンダーライブラリ
  import 'flatpickr/dist/flatpickr.min.css' // FlatpickrのCSS
  import { Japanese } from 'flatpickr/dist/l10n/ja.js' // 日本語対応
  import { openPath, openUrl } from '@tauri-apps/plugin-opener'
	import { startScan, scanProgress } from '$lib/scanController'
	import { statusStore } from '../statusStore'
  import { init, locale, register, t, waitLocale } from 'svelte-i18n'
	import { updateFromScanProgress } from '../statusStore'
	
  let isUpdateDBWhenInit = false
  let config: Config | null = null

  // 初期化が完了したかどうかを示すフラグ
  $: updateDBWhenInit.subscribe((value) => {
    isUpdateDBWhenInit = value
  })

  $: configStore.subscribe((value) => {
    config = value
  })

  // ストアから現在のステートを取得
  let status = {
    message: '',
    progress: null as number | null,
    type: 'info',
    isVisible: true,
  }
  $: statusStore.subscribe((value) => (status = value))

  let thumbnails: any[] = [] // サムネイルデータ
  $: thumbnailStore.subscribe((value) => (thumbnails = value))
  let activePage: string = 'thumbnails' // アクティブなページ
  let offset = 0 // 現在の取得済み件数（開始位置）
  const limit = 20 // 一度に取得する件数
	const INDEX_DB_ID = 'imageCache'

  let isLoading = false // データ取得中のフラグ
  let allLoaded = false // 全データ取得済みフラグ
  let statusMessage: string = '' // ステータスバーのメッセージ
	
  let selectedImageInfo: {
    metadata: any | null
    data_url: any | null
    file_created_at: any | null
    filePath: string | null
    dbid: string | null
  } = {
    metadata: null,
    data_url: null,
    file_created_at: null,
    filePath: null,
    dbid: null,
  }
  let isMetadataPopupVisible = false // メタデータ表示用ポップアップの可視状態

  let isLocaleReady: boolean = false
  let isConfigReady: boolean = false
  async function initializeI18n() {
    register('en', () => import('$lib/locale/en.json'))
    register('ja', () => import('$lib/locale/ja.json'))
    init({
      fallbackLocale: 'en',
      initialLocale: 'ja',
    })
    await waitLocale() // 初期化が完了するまで待機
    isLocaleReady = true
  }

	let unsubscribeScan: (() => void) | null = null

	//Scan locking
	let scanDone = true
	let activeScanId: string | null = null
	let lockedScanTotal: number | null = null

	onMount(async () => {
		// ─────────────────────────────────────────────
		// Scan progress subscription (total locked per scan)
		// ─────────────────────────────────────────────
		unsubscribeScan = scanProgress.subscribe((p) => {
			// No active scan
			if (!p || p.scanId === null) {
				scanDone = true

				statusStore.set({
					type: 'info',
					message: '',
					progress: null,
					isVisible: false,
				})

				activeScanId = null
				lockedScanTotal = null
				return
			}

			// New scan → reset lock
			if (p.scanId !== activeScanId) {
				activeScanId = p.scanId
				lockedScanTotal = null
			}

			// Lock total once it becomes valid (> 0)
			if (lockedScanTotal === null && p.total > 0) {
				lockedScanTotal = p.total
			}

			scanDone = p.done || p.cancelled

			// ─────────────────────────────────────────────
			// STATUS BAR OUTPUT (NO 0 / 0 EVER)
			// ─────────────────────────────────────────────
			if (p.cancelled) {
				statusStore.set({
					type: 'error',
					message: $t('app.messages.scan_cancelled') ?? 'Scan cancelled.',
					progress: null,
					isVisible: true,
				})
				return
			}

			if (p.done) {
				statusStore.set({
					type: 'success',
					message:
						$t('app.messages.database_update_complete') ??
						'Database update complete.',
					progress: null,
					isVisible: true,
				})
				return
			}

			//Still scanning
			if (!lockedScanTotal || lockedScanTotal <= 0 || (p.processed ?? 0) === 0) {
				statusStore.set({
					type: 'info',
					message: $t('app.messages.scanning') ?? 'Scanning…',
					progress: null,
					isVisible: true,
				})
				return
			}

			// NORMAL PHASE show real progress
			const processed = Math.min(p.processed ?? 0, lockedScanTotal)

			statusStore.set({
				type: 'info',
				message: `${$t('app.messages.loading_thumbnails')} ${processed} / ${lockedScanTotal}`,
				progress: processed / lockedScanTotal,
				isVisible: true,
			})
		})

		// ─────────────────────────────────────────────
		// App initialization
		// ─────────────────────────────────────────────
		const isInitialized = await getInitialSetupState()
		if (!isInitialized) {
			statusMessage = $t('app.messages.require_initial_setup')
			await goto('/settings')
			return
		}

		await initializeI18n()

		config = await getConfig()
		isConfigReady = true
		locale.set(config.feature_flags.language)

		initializeFlatpickr()

		if (!isUpdateDBWhenInit && config?.feature_flags.update_db_when_startup) {
			await updateDatabase()
			updateDBWhenInit.set(true)
		}

		// ─────────────────────────────────────────────
		// Initial thumbnail load (scan-safe)
		// ─────────────────────────────────────────────
		if (thumbnails.length === 0 && scanDone) {
			await loadAllThumbnails()
		} else if (thumbnails.length > 0) {
			groupedThumbnails = groupThumbnailsByDirectory(thumbnails)
		}
	})

	onDestroy(() => {
		unsubscribeScan?.()
	})
	
	let lastScanDone = true

	$: if (scanDone && !lastScanDone) {
		// Scan just transitioned from running → finished
		allLoaded = false
		offset = 0
		thumbnails = []
		groupedThumbnails = {}

		loadAllThumbnails()
	}

	$: lastScanDone = scanDone

	async function updateDatabase() {
		const folders = await getAllFolders()
		const mappedFolders = folders.map(f => f.path)

		scanDone = false
		allLoaded = false
		offset = 0

		await startScan(mappedFolders)
	}

  let groupedThumbnails: { [key: string]: any[] } = {} // グループ化されたサムネイルデータ
  let thumbnailKeys: string[] = [] // キーリスト
  let thumbnailProcessed = false

  function groupThumbnailsByDirectory(thumbnails: any[]) {
    const grouped: { [key: string]: any[] } = {}

    for (const thumbnail of thumbnails) {
      const filePath = thumbnail[0] // サムネイルのファイルパス
      const directory = filePath.substring(0, filePath.lastIndexOf('\\')) // ディレクトリを抽出

      // ディレクトリごとにデータをグループ化
      if (!grouped[directory]) {
        grouped[directory] = []
      }
      grouped[directory].push(thumbnail)
    }
    thumbnailKeys = Object.keys(grouped)
    thumbnailProcessed = true
    return grouped
  }

  // 全サムネイルを順次ロード
  async function loadAllThumbnails(): Promise<void> {
	  allLoaded = false
		offset = 0
    groupedThumbnails = {}
    thumbnails = []
		let safety = 0
		while (!allLoaded && safety++ < 10000) {
			await loadMoreThumbnails()
			groupedThumbnails = groupThumbnailsByDirectory(thumbnails)
		}
    thumbnailStore.set(thumbnails)
  }

	// サムネイルを順次ロードする
	async function loadMoreThumbnails(): Promise<void> {
		if (isLoading || allLoaded) return

		isLoading = true
		statusMessage = $t('app.titles.load_thumbnails') // ローカル表示用のみ

		try {
			const newThumbnails = await getThumbnailsChunk(offset, limit)

			if (newThumbnails.length === 0) {
				allLoaded = true
				statusMessage = $t('app.messages.all_thumbnails_loaded').replace(
					'{0}',
					thumbnails.length.toString()
				)
			} else {
				thumbnails = [...thumbnails, ...newThumbnails]
				offset += limit
				statusMessage = $t('app.messages.thumbnails_displayed').replace(
					'{0}',
					thumbnails.length.toString()
				)
			}
		} catch (error) {
			console.error('ロードエラー:', error)

			statusMessage = $t('app.messages.thumbnail_load_error')

			// ❗ hard failure → global status only here
			statusStore.set({
				type: 'error',
				message: `${statusMessage}: ${(error as Error).message}`,
				isVisible: true,
				progress: null,
			})
		} finally {
			isLoading = false
		}
	}

  async function openImageFile(filePath: string) {
    try {
      await openPath(filePath) // ファイルパスを OS 標準ビューアに渡す
    } catch (error) {
      console.error('画像を標準ビューアで開けませんでした:', error)
      statusMessage = `画像を標準ビューアで開けませんでした。: ${filePath}`
      statusStore.set({
        type: 'error',
        message: `画像を標準ビューアで開けませんでした。: ${filePath}`,
        isVisible: true,
        progress: null,
      })
    }
  }

  async function handleImageClick(dbid: string, filePath: string) {
    try {
      const { metadata, data_url, file_created_at } = await getMetadata(
        dbid,
        filePath
      )
      selectedImageInfo = {
        metadata,
        data_url,
        file_created_at,
        filePath,
        dbid,
      }
      isMetadataPopupVisible = true
    } catch (error) {
      console.error('メタデータ取得エラー:', error)
      statusStore.set({
        type: 'error',
        message: $t('app.messages.metadata_loading_error'),
        isVisible: true,
        progress: null,
      })
      alert($t('app.messages.metadata_loading_error'))
    }
  }

  let isActionModalVisible = false // モーダルの可視状態
  let selectedActionType = '' // 選択された種類（world または player）
  let selectedActionId = '' // 選択された ID
  let selectedActionName = ''

  function handleWorldClick(id: string, name: string) {
    selectedActionType = 'world' // 種類を指定
    selectedActionId = id // 選択した World ID を保存
    selectedActionName = name
    isActionModalVisible = true // モーダルを表示
  }

  function handlePlayerClick(id: string, name: string) {
    selectedActionType = 'player' // 種類を指定
    selectedActionId = id // 選択した Player ID を保存
    selectedActionName = name
    isActionModalVisible = true // モーダルを表示
  }

  // 外部 URL を開く処理
  async function openExternalProfile(type: string, id: string) {
    const url =
      type === 'world'
        ? `https://vrchat.com/home/world/${id}/info`
        : `https://vrchat.com/home/user/${id}`
    try {
      if (!id) {
        throw new Error(
          $t('app.messages.missing_id_error').replace(
            '{0}',
            type === 'world' ? $t('app.types.world') : $t('app.types.player')
          )
        )
      }
      await openUrl(url)
    } catch (error) {
      console.error('外部 URL を開けませんでした:', error)
      statusMessage = $t('app.messages.open_external_url_error')
      statusStore.set({
        type: 'error',
        message: `${statusMessage}: ${(error as Error).message}`,
        isVisible: true,
        progress: null,
      })
    } finally {
      isActionModalVisible = false
    }
  }

  // 自動生成された検索条件に追加する処理
  function addToSearchConditions(type: string, id: string) {
    conditions.push({
      logic: 'AND',
      field: type === 'player' ? 'player' : 'world',
      operator: '=',
      value: id,
    })
    conditions = [...conditions]
    console.log('検索条件に追加:', { type, id })
    isActionModalVisible = false // モーダルを閉じる
    isSearchVisible = true
    isMetadataPopupVisible = false
  }

  let conditions = [{ logic: 'AND', field: 'world', operator: '=', value: '' }]

  // カレンダーのオプション
  // Flatpickrのオプション設定
  const calendarOptions = {
    locale: Japanese,
    enableTime: true,
    time_24hr: true,
    dateFormat: 'Z',
    altInput: true, // カスタム表示用入力フィールドを有効化
    altFormat: 'Y-m-d H:i',
    defaultDate: new Date(),
    onChange: (selectedDates: Date[], dateStr: string) => {
      // 条件の値に選択した日付を反映
      const condition = conditions.find(
        (condition) => condition.field === 'created_at'
      )
      if (condition) {
        condition.value = dateStr
      }
    },
  }

  // Flatpickrの実行処理
  function initializeFlatpickr() {
    const elements = document.querySelectorAll(`[id^="created-at-picker-"]`)
    elements.forEach((element) => {
      flatpickr(element, calendarOptions)
    })
  }

  let calendarInstances = new Map<number, any>() // Flatpickrインスタンスを保存するマップ

  // 条件が変わった場合でも既存インスタンスを保持
  afterUpdate(() => {
    const elements = document.querySelectorAll(`[id^="created-at-picker-"]`)

    elements.forEach((element, index) => {
      // インスタンスがまだ初期化されていない場合のみFlatpickrを初期化
      if (!calendarInstances.has(index)) {
        const flatpickrInstance = flatpickr(element, {
          ...calendarOptions,
          defaultDate: conditions[index]?.value || new Date(), // 条件の値がある場合はそれを設定
          onChange: (selectedDates: Date[], dateStr: string) => {
            // 選択した日付を条件に反映
            conditions[index].value = dateStr
          },
        })
        calendarInstances.set(index, flatpickrInstance)
      } else {
        // 既存インスタンスが存在する場合、値を更新する
        const instance = calendarInstances.get(index)
        if (conditions[index]?.value) {
          instance.setDate(conditions[index].value, false) // 値を更新（イベントをトリガーしない）
        }
      }
    })

    // 不要なインスタンスを削除
    if (calendarInstances.size > elements.length) {
      Array.from(calendarInstances.keys()).forEach((key) => {
        if (!elements[key]) {
          const instance = calendarInstances.get(key)
          instance.destroy() // Flatpickrインスタンスを削除
          calendarInstances.delete(key) // キャッシュからも削除
        }
      })
    }
  })
  // 新しい条件を追加
  function addCondition() {
    conditions.push({ logic: 'AND', field: '', operator: '=', value: '' })
    conditions = [...conditions]
  }

  // 条件を削除
  function removeCondition(index: number) {
    if (conditions.length > 1) {
      conditions.splice(index, 1)
      conditions = [...conditions]
    }
  }

  function clearCondition() {
    const len = conditions.length
    // 検索条件を初期の状態にリセット
    conditions = [{ logic: 'AND', field: 'world', operator: '=', value: '' }]
    conditions = [...conditions]
    if (len > 0) {
      handleReloadButton()
    }
  }

	// サーバーにクエリを送信して検索（backend）
	async function handleSearch() {
		try {
			if (isLoading) return

			// Build backend conditions (only non-empty)
			const mapped = conditions
				.map((c, idx) => ({
					logic: (idx === 0 ? 'AND' : (c.logic ?? 'AND')).toUpperCase(),
					field: c.field,
					operator: (c.operator ?? 'eq').toUpperCase(), // eq -> EQ, like -> LIKE, etc.
					value: (c.value ?? '').trim(),
				}))
				.filter((c) => c.value.length > 0)

			if (mapped.length === 0) {
				statusStore.set({
					type: 'error',
					message:
						($t('app.messages.missing_search_value') as string) ??
						'Please enter a search value.',
					isVisible: true,
					progress: null,
				})
				return
			}

			isLoading = true
			statusMessage = ($t('app.messages.searching') as string) ?? 'Searching...'

			// Backend search (fast + no UI freeze)
			const results = (await searchImage(mapped)) as any[]

			if (!results || results.length === 0) {
				statusStore.set({
					type: 'error',
					message: $t('app.messages.search_no_results'),
					isVisible: true,
					progress: null,
				})
				return
			}

			// results already match your thumbnail shape: (file_path, data_url, uuid)
			thumbnails = results
			thumbnailStore.set(results)
			groupedThumbnails = groupThumbnailsByDirectory(results)

			statusStore.set({
				type: 'success',
				message: ($t('app.messages.search_complete') as string)
					.replace('{0}', results.length.toString()),
				isVisible: true,
				progress: null,
			})
		} catch (error) {
			console.error('Search error:', error)
			statusStore.set({
				type: 'error',
				message: `${($t('app.messages.search_error') as string) ?? 'Search failed'}: ${
					(error as Error).message
				}`,
				isVisible: true,
				progress: null,
			})
		} finally {
			isLoading = false
		}
	}


  let isSearchVisible = false // 検索フォームの表示状態

  // 検索フォームの表示状態を切り替える
  function toggleSearch() {
    isSearchVisible = !isSearchVisible
  }

	function handleReloadButton() {
		if (!isLoading && scanDone) {
			allLoaded = false
			offset = 0
			loadAllThumbnails()
		}
	}

  function handleFieldChange(index: number) {
    const condition = conditions[index]

    // 各フィールドに応じてデフォルト値を初期化
    if (condition.field === 'created_at') {
      condition.operator = 'eq' // デフォルトのオペレーター
      condition.value = '' // 日付を空にする
    } else if (condition.field === 'world' || condition.field === 'player') {
      condition.operator = 'eq' // デフォルトのオペレーター
      condition.value = '' // 入力値を空にする
    }

    // 条件を更新して再レンダリングをトリガー
    conditions = [...conditions]
  }
</script>

<Layout {activePage} {statusMessage} {thumbnailKeys} {thumbnailProcessed}>
  {#if isLocaleReady && isConfigReady}
    <div class="container-top">
      <h1 id="thumbnails">{$t('app.thumbnail_list')}</h1>
    </div>

    <!-- トップのボタン -->
    <div class="top-button-container">
      <button on:click={toggleSearch} class="search-toggle-button"
        >🔍 {$t('app.search_button')}</button
      >
      <button on:click={handleReloadButton} class="search-toggle-button"
        >🔄️ {$t('app.reload_button')}</button
      >
    </div>

    <!-- 検索フォーム -->
    {#if isSearchVisible}
      <div class="search-builder">
        <div class="search-title">
          <h4>{$t('app.search_conditions')}</h4>
          <button on:click={toggleSearch} class="close-button"
            >✖ {$t('app.close')}</button
          >
        </div>

        {#each conditions as condition, index}
          <div class="condition-row">
            {#if index > 0}
              <select bind:value={condition.logic}>
                <option value="AND">AND</option>
                <option value="OR">OR</option>
              </select>
            {/if}

            <select
              bind:value={condition.field}
              on:change={() => handleFieldChange(index)}
            >
              <option value="world">{$t('app.fields.world')}</option>
              <option value="player">{$t('app.fields.player')}</option>
              <option value="created_at">{$t('app.fields.created_at')}</option>
            </select>

            {#if condition.field === 'world' || condition.field === 'player'}
              <select bind:value={condition.operator}>
                <option value="eq">{$t('app.operators.equal')}</option>
                <option value="ne">{$t('app.operators.not_equal')}</option>
                <option value="like">{$t('app.operators.contains')}</option>
              </select>
            {:else}
              <select bind:value={condition.operator}>
                <option value="eq">{$t('app.operators.equal')}</option>
                <option value="ne">{$t('app.operators.not_equal')}</option>
                <option value="gt">{$t('app.operators.greater_than')}</option>
                <option value="ge"
                  >{$t('app.operators.greater_than_or_equal')}</option
                >
                <option value="lt">{$t('app.operators.less_than')}</option>
                <option value="le"
                  >{$t('app.operators.less_than_or_equal')}</option
                >
              </select>
            {/if}

            {#if condition.field === 'created_at'}
              <input
                id="created-at-picker-{index}"
                type="text"
                placeholder={$t('app.select_date')}
              />
            {:else}
              <input
                type="text"
                bind:value={condition.value}
                placeholder={$t('app.search_placeholder')}
              />
            {/if}

            <button on:click={() => removeCondition(index)}
              >{$t('app.remove_condition')}</button
            >
          </div>
        {/each}

        <div class="search-actions">
          <button class="search-clear-button" on:click={clearCondition}
            >{$t('app.clear_condition')}</button
          >
          <button on:click={addCondition}>{$t('app.add_condition')}</button>
          <button on:click={handleSearch}>{$t('app.search_button')}</button>
        </div>
      </div>
    {/if}

		<!-- サムネイル表示 -->
		<div class="directories">
			{#each Object.keys(groupedThumbnails) as directory}
				<div class="directory">
					<h3 class="directory-title" id={directory}>{directory}</h3>

					<div class="grid" role="grid" aria-labelledby={directory}>
						{#each groupedThumbnails[directory] as thumbnail}
							<button
								type="button"
								class="grid-item"
								role="gridcell"
								on:click={() => handleImageClick(INDEX_DB_ID, thumbnail[0])}
							>
								<img
									src={thumbnail[1]}
									alt={thumbnail[0]}
									loading="lazy"
								/>
							</button>
						{/each}
					</div>
				</div>
			{/each}
		</div>

		<!-- メタデータポップアップ -->
		{#if isMetadataPopupVisible}
			<div class="popup">
				<div class="popup-content">
					<h4>{$t('app.world_info')}</h4>

					<div class="popup-image">
						<img
							src={selectedImageInfo.data_url}
							alt={selectedImageInfo.filePath}
						/>
					</div>

					{#if selectedImageInfo.filePath}
						<div class="file-info">
							<p>
								<strong>{$t('app.file_name')}:</strong>
								<button
									type="button"
									class="file-link"
									on:click={() => {
										if (selectedImageInfo.filePath) {
											openImageFile(selectedImageInfo.filePath)
										}
									}}
								>
									{selectedImageInfo.filePath}
								</button>
							</p>
						</div>
					{/if}

					{#if selectedImageInfo.metadata}
						<h4>{$t('app.world_info')}</h4>

						<button
							type="button"
							class="world-card"
							on:click={() =>
								handleWorldClick(
									selectedImageInfo.metadata.world?.id,
									selectedImageInfo.metadata.world?.name
								)
							}
						>
							<p class="world-name">
								{selectedImageInfo.metadata.world?.name || '不明'}
							</p>
						</button>

						<h4>{$t('app.player_list')}</h4>

						<div class="metadata-players-grid">
							{#if selectedImageInfo.metadata.players?.length > 0}
								{#each selectedImageInfo.metadata.players as player}
									<button
										type="button"
										class="player-card"
										on:click={() =>
											handlePlayerClick(player.displayName, player.id)
										}
									>
										<strong class="player-name">
											{player.displayName || '不明'}
										</strong>
									</button>
								{/each}
							{:else}
								<p>{$t('app.no_player_info')}</p>
							{/if}
						</div>
					{:else}
						<p>{$t('app.no_metadata')}</p>
					{/if}

					<button
						type="button"
						class="close-button"
						on:click={() => (isMetadataPopupVisible = false)}
					>
						{$t('app.close')}
					</button>
				</div>
			</div>
		{/if}

    <!-- アクションモーダル -->
    {#if isActionModalVisible}
      <div class="modal">
        <div class="modal-content">
          <h3>{$t('app.modal_title.' + selectedActionType)}</h3>
          <p>
            {selectedActionType === 'world'
              ? $t('app.fields.world')
              : $t('app.fields.player')}: {selectedActionName}
          </p>

          <div class="modal-actions">
            <button
              on:click={() =>
                openExternalProfile(selectedActionType, selectedActionId)}
            >
              {$t('app.external_profile')}
            </button>
            <button
              on:click={() =>
                addToSearchConditions(selectedActionType, selectedActionId)}
            >
              {$t('app.use_in_search')}
            </button>
          </div>

          <button on:click={() => (isActionModalVisible = false)}
            >{$t('app.close')}</button
          >
        </div>
      </div>
    {/if}
  {/if}
</Layout>

<style>
  .directories {
    display: flex;
    flex-direction: column;
    gap: 2rem; /* ディレクトリ間の余白 */
    margin: auto 1rem;
  }

  .directory-title {
    font-size: 1.2rem;
    font-weight: bold;
    color: #333;
    margin-bottom: 1rem;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 1rem;
  }

	.grid-item {
		all: unset;
		position: relative;
		aspect-ratio: 16 / 9;
		overflow: hidden;
		border-radius: 12px;
		display: flex;
		justify-content: center;
		align-items: center;
		background-color: #202020;
		cursor: pointer;

		transition:
			transform 0.3s ease,
			box-shadow 0.3s ease,
			outline 0.3s ease;

		box-shadow: 0 2px 6px rgba(0, 0, 0, 0.15);
	}

	.grid-item:hover {
		transform: scale(1.05);
		box-shadow: 0 6px 15px rgba(0, 0, 0, 0.25);
	}

	.grid-item:focus-visible {
		outline: 2px solid #7da2f8;
		outline-offset: 2px;
	}

	.grid-item img {
		max-width: 100%;
		max-height: 100%;
		width: 150%;
		object-fit: contain;
		pointer-events: none;
	}

  .popup {
    position: fixed;
    top: 0;
    left: 0;
    background-color: rgba(0, 0, 0, 0.7);
    width: 100vw;
    height: 100vh;
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 1000;
  }

  .popup-content {
    background: #fff;
    padding: 2rem;
    border-radius: 8px;
    max-width: 80%;
    max-height: 80%;
    overflow-y: auto;
  }

  .popup-image {
    position: relative;
    aspect-ratio: 16 / 9;
    overflow: hidden;
    border: 1px solid #ccc;
    border-radius: 4px;
    display: flex;
    justify-content: center;
    align-items: center;
    width: 50vw;
    height: auto;
    margin: auto;
  }

  @media screen and (max-width: 959px) {
    .popup-image {
      width: 60vw;
    }
  }
  @media screen and (max-width: 480px) {
    .popup-image {
      width: 70vw;
    }
  }

  .popup-image img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }

  .popup button {
    display: block;
    margin: 1rem auto 0;
  }

	.world-card {
		all: unset;
		display: flex;
		align-items: center;
		justify-content: center;
		background: linear-gradient(135deg, #93beff, #77d4ff);
		border-radius: 8px;
		padding: 2rem 1rem;
		text-align: center;
		color: white;
		cursor: pointer;
		margin: 1rem 2rem;
		box-shadow: 0 4px 8px rgba(0, 0, 0, 0.2);
	}

	.world-card:hover {
		transform: scale(1.03);
	}

	.world-card:focus-visible {
		outline: 2px solid #ffffff;
		outline-offset: 2px;
	}

  .world-name {
    font-size: 1.3rem; /* フォントサイズを拡大 */
    font-weight: bold;
    letter-spacing: 1px; /* テキストにスペースを追加 */
    text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.3); /* 少しの影を追加して目立たせる */
    margin: 0;
  }

  .metadata-players-grid {
    display: grid;
    grid-template-columns: repeat(
      auto-fit,
      minmax(150px, 1fr)
    ); /* 自動的に列設定 */
    gap: 1rem;
    margin-top: 1rem;
  }

	.player-card {
		all: unset;
		background-color: #f7f7f7;
		border: 1px solid #ddd;
		border-radius: 5px;
		padding: 1rem;
		text-align: center;
		cursor: pointer;
		box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
		transition:
			transform 0.2s ease,
			box-shadow 0.2s ease;
	}

	.player-card:hover {
		transform: scale(1.05);
		box-shadow: 0 4px 8px rgba(0, 0, 0, 0.2);
	}

	.player-card:focus-visible {
		outline: 2px solid #007bff;
		outline-offset: 2px;
	}

  .player-name {
    font-size: 1rem;
    color: #333;
  }

  .search-builder {
    /*background-color: #f8f9fa;*/
    background-color: rgba(248, 249, 250, 0.98);
    padding: 0.8rem;
    border-radius: 8px;
    position: fixed;
    z-index: 10;
    bottom: 4rem;
    right: 2rem;
    border: #333;
    box-shadow: 0 0 0.2rem black;
    max-width: 80vw;
  }

  .search-title {
    display: flex;
    flex-direction: row;
    justify-content: space-between;
    align-items: center;
    margin: 1.33rem;
    margin-right: 0 !important;
  }

  .search-title h4 {
    margin: 0;
  }

  .condition-row {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 1rem;
    justify-content: space-between;
  }

  .search-builder select,
  .search-builder input {
    padding: 0.5rem;
    font-size: 0.8rem;
    border: 1px solid #ccc;
    border-radius: 4px;
  }

  .search-builder button {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 4px;
    color: white;
    cursor: pointer;
    font-size: 0.8rem;
  }

  .search-builder button:hover {
    opacity: 0.8;
  }

  .search-actions {
    display: flex;
    gap: 1rem;
    justify-content: flex-end;
  }

  .search-actions button {
    background-color: #007bff;
    color: white;
  }

  .search-clear-button {
    background-color: rgba(255, 61, 61, 0) !important;
    color: #ff3d3d !important;
    border: 1px solid #ff3d3d !important;
    border-radius: 0.3rem !important;
    transition: all 0.3s ease; /* 追加部分 */
  }

  .search-clear-button:hover {
    background-color: rgba(255, 61, 61, 1) !important;
    color: white !important;
    border: 1px solid #ff3d3d !important;
    border-radius: 0.3rem !important;
  }

  .condition-row button {
    background-color: #ff3d3d;
    max-width: 10vw;
    text-wrap: nowrap;
    text-overflow: clip;
  }

  .condition-row select,
  .condition-row input {
    max-width: 10vw;
  }

  .search-toggle-button {
    background-color: #007bff;
    color: white;
    padding: 0.4rem 1rem;
    border: none;
    border-radius: 4px;
    font-size: 0.9rem;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    margin-right: 1rem;
  }

  .search-toggle-button:hover {
    opacity: 0.8;
  }

  .container-top {
    display: flex;
    flex-direction: row;
    justify-content: space-between;
    align-items: center;
  }
  .top-button-container {
    display: flex;
    flex-direction: row;
    justify-content: space-between;
    position: fixed;
    z-index: 10;
    top: 4rem;
    right: 2.5rem;
  }

	.file-info {
		margin-top: 1rem;
		text-align: center; /* 水平配置 */
		font-size: 0.9rem;
	}

	.file-info .file-link {
		background: none;
		border: none;
		padding: 0;
		margin: 0;
		color: #007bff; /* リンクの色 */
		text-decoration: none;
		font-weight: bold;
		cursor: pointer;
		transition: color 0.2s ease;
	}

	.file-info .file-link:hover {
		color: #0056b3; /* ホバー時の色 */
		text-decoration: underline; /* ホバー時に下線を表示 */
	}

	.file-info .file-link:focus-visible {
		outline: 2px solid #007bff;
		outline-offset: 2px;
	}

  .modal {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background-color: rgba(0, 0, 0, 0.7);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 1000;
  }

  .modal-content {
    background: #fff;
    padding: 2rem;
    border-radius: 8px;
    width: 300px;
    text-align: center;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
  }

  .modal-actions {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    margin: 1rem 0;
  }

  .modal-actions button {
    background-color: #007bff;
    color: white;
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.9rem;
  }

  .modal-actions button:hover {
    opacity: 0.8;
  }

  .close-button {
    background-color: #ff3d3d !important;
    color: white;
    padding: 0.3rem 0.8rem;
    border: none;
    border-radius: 4px;
    font-size: 0.8rem;
    cursor: pointer;
  }

  .close-button:hover {
    opacity: 0.8;
  }
</style>
