import { useEffect, useMemo, useRef, useState } from 'react'
import './WorkspaceApp.css'
import LoginProviderPopup from './LoginProviderPopup'
import { withBasePath } from './pathUtils'

const FRONTEND_VERSION = import.meta.env.VITE_APP_VERSION || '2.1.1'

function SunIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="12" cy="12" r="4.25" fill="currentColor" />
      <g stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
        <path d="M12 2.5v2.7" />
        <path d="M12 18.8v2.7" />
        <path d="M2.5 12h2.7" />
        <path d="M18.8 12h2.7" />
        <path d="m5.3 5.3 1.9 1.9" />
        <path d="m16.8 16.8 1.9 1.9" />
        <path d="m16.8 7.2 1.9-1.9" />
        <path d="m5.3 18.7 1.9-1.9" />
      </g>
    </svg>
  )
}

function MoonIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M15.7 3.2a8.8 8.8 0 1 0 5.1 15.7A9.6 9.6 0 0 1 15.7 3.2Z"
        fill="currentColor"
      />
    </svg>
  )
}

function FolderIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M3.8 7.2a2.2 2.2 0 0 1 2.2-2.2h4l1.8 1.8h6.2a2.2 2.2 0 0 1 2.2 2.2v7.8a2.2 2.2 0 0 1-2.2 2.2H6a2.2 2.2 0 0 1-2.2-2.2V7.2Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinejoin="round"
      />
    </svg>
  )
}

function DownloadIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M12 4.5v9.2m0 0 3.4-3.4M12 13.7l-3.4-3.4M5 18.5h14"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

function OpenFileIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M5.5 19.2V6.8A1.8 1.8 0 0 1 7.3 5h5.8l3.4 3.4v10.8A1.8 1.8 0 0 1 14.7 21H7.3a1.8 1.8 0 0 1-1.8-1.8Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinejoin="round"
      />
      <path
        d="M13.1 5v3.6h3.6"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinejoin="round"
      />
    </svg>
  )
}

function RefreshIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M18 8.2A7 7 0 1 0 19 15"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <path
        d="M18.2 4.8v4.6h-4.6"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

function SpinnerIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <circle
        cx="12"
        cy="12"
        r="8"
        fill="none"
        stroke="currentColor"
        strokeOpacity="0.22"
        strokeWidth="2"
      />
      <path
        d="M12 4a8 8 0 0 1 8 8"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
      />
    </svg>
  )
}

function ShareIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="9" cy="9" r="2.6" fill="none" stroke="currentColor" strokeWidth="1.7" />
      <circle cx="16.2" cy="10.4" r="2.2" fill="none" stroke="currentColor" strokeWidth="1.7" />
      <path
        d="M4.8 18.3c0-2.4 2.1-4.2 4.8-4.2s4.8 1.8 4.8 4.2"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
      />
      <path
        d="M14.4 17.2c.4-1.5 1.8-2.6 3.6-2.6 1 0 1.9.3 2.6.9"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
      />
    </svg>
  )
}

function GlobeIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="12" cy="12" r="8" fill="none" stroke="currentColor" strokeWidth="1.7" />
      <path
        d="M4.8 9.2h14.4M4.8 14.8h14.4M12 4a12.8 12.8 0 0 1 3.3 8A12.8 12.8 0 0 1 12 20a12.8 12.8 0 0 1-3.3-8A12.8 12.8 0 0 1 12 4Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

function LockIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M7.2 10.2V8.7a4.8 4.8 0 0 1 9.6 0v1.5M6.8 10.2h10.4a1.8 1.8 0 0 1 1.8 1.8v6.2a1.8 1.8 0 0 1-1.8 1.8H6.8A1.8 1.8 0 0 1 5 18.2V12a1.8 1.8 0 0 1 1.8-1.8Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

function LogoutIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M10 4.8H7.8A1.8 1.8 0 0 0 6 6.6v10.8a1.8 1.8 0 0 0 1.8 1.8H10"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M13.5 8.2 18 12l-4.5 3.8M9.5 12H18"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

function BrowseIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M4.5 12h6.5m-6.5 0 2.6-2.6M4.5 12l2.6 2.6"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M19.5 12H13m6.5 0-2.6-2.6m2.6 2.6-2.6 2.6"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M9.2 6.2h5.6a3 3 0 0 1 3 3v5.6a3 3 0 0 1-3 3H9.2a3 3 0 0 1-3-3V9.2a3 3 0 0 1 3-3Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinejoin="round"
      />
    </svg>
  )
}

function PenIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M4 16.8V20h3.2l9.7-9.7-3.2-3.2L4 16.8Zm11.7-10.9 1.2-1.2a1.8 1.8 0 0 1 2.6 0l.8.8a1.8 1.8 0 0 1 0 2.6l-1.2 1.2-3.4-3.4Z"
        fill="currentColor"
      />
    </svg>
  )
}

function PlusIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M12 5.5v13M5.5 12h13"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.9"
        strokeLinecap="round"
      />
    </svg>
  )
}

function TrashIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M4.5 7h15m-11 0V5.8c0-.7.6-1.3 1.3-1.3h4.4c.7 0 1.3.6 1.3 1.3V7m-8.9 0 .7 10.2c.1 1 1 1.8 2 1.8h5.4c1 0 1.9-.8 2-1.8L17.4 7M10 10.2v5.8m4-5.8v5.8"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

function ChevronIcon({ open = false }) {
  return (
    <svg
      viewBox="0 0 24 24"
      aria-hidden="true"
      style={{ transform: open ? 'rotate(90deg)' : 'rotate(0deg)' }}
    >
      <path
        d="m9 6 6 6-6 6"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

function publicPath(relativePath) {
  return withBasePath(relativePath)
}

function apiPath(relativePath) {
  return withBasePath(relativePath)
}

function navigateTo(relativePath) {
  const target = publicPath(relativePath.startsWith('/') ? relativePath.slice(1) : relativePath)
  window.history.pushState({}, '', target)
  window.dispatchEvent(new PopStateEvent('popstate'))
}

async function readError(response) {
  const raw = await response.text()
  if (!raw) {
    return `Request failed with status ${response.status}.`
  }

  try {
    const parsed = JSON.parse(raw)
    if (typeof parsed.error === 'string' && parsed.error.trim()) {
      return parsed.error
    }
  } catch {
    // Ignore parse errors and fall back to plain text.
  }

  return raw.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim()
}

async function request(relativePath, options = {}) {
  const response = await fetch(apiPath(relativePath), {
    credentials: 'same-origin',
    ...options,
    headers: {
      Accept: 'application/json',
      ...(options.body instanceof FormData ? {} : options.body ? { 'Content-Type': 'application/json' } : {}),
      ...(options.headers || {}),
    },
  })

  if (!response.ok) {
    throw new Error(await readError(response))
  }

  return response
}

async function requestJson(relativePath, options = {}) {
  const response = await request(relativePath, options)
  if (response.status === 204) {
    return null
  }
  return response.json()
}

async function requestText(relativePath, options = {}) {
  const response = await request(relativePath, options)
  return response.text()
}

function triggerDownload(filename, text, mimeType = 'text/plain;charset=utf-8') {
  const blob = new Blob([text], { type: mimeType })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = filename
  anchor.click()
  URL.revokeObjectURL(url)
}

function formatUserLabel(user) {
  return user.display_name || user.username || user.email
}

function parseProjectId(routePath) {
  const match = routePath.match(/^\/app\/projects\/([^/]+)\/?$/)
  return match ? decodeURIComponent(match[1]) : null
}

function roleLabel(role) {
  if (role === 'owner') return 'Owner'
  if (role === 'editor') return 'Editor'
  return 'Viewer'
}

function visibilityLabel(visibility) {
  return visibility === 'public' ? 'Public' : 'Private'
}

function venueFieldLabel(fieldName) {
  return fieldName === 'booktitle' ? 'Booktitle' : 'Journal'
}

function canEdit(role) {
  return role === 'owner' || role === 'editor'
}

function isOwner(role) {
  return role === 'owner'
}

function folderDisplayName(folder) {
  return folder?.name || 'Folder'
}

function processingPhase(task, tick) {
  if (!task) {
    return null
  }

  const elapsed = Math.max(0, tick - task.startedAt)
  const phaseIndex = Math.min(
    Math.floor(elapsed / 1600),
    task.kind === 'upload' ? 4 : 4,
  )

  const phases =
    task.kind === 'upload'
      ? [
          'Uploading PDF to the server…',
          'Extracting PDF text and front matter…',
          'Resolving DOI and metadata sources…',
          'Generating BibTeX entry and tags…',
          'Refreshing the library view…',
        ]
      : [
          'Reading the stored PDF…',
          'Extracting PDF text and front matter…',
          'Resolving DOI and title-based metadata…',
          'Generating BibTeX entry and tags…',
          'Refreshing the document details…',
        ]

  return phases[phaseIndex]
}

function matchesDocumentSearch(document, searchTerm) {
  if (!searchTerm) {
    return true
  }

  return [
    document.title,
    document.authors_text,
    document.doi,
    document.citation_key,
    document.original_filename,
  ]
    .filter(Boolean)
    .some((value) => value.toLowerCase().includes(searchTerm))
}

function createFolderTree(folders) {
  const rootFolder = folders.find((folder) => folder.is_hidden && !folder.parent_folder_id) || null
  const visibleFolders = folders.filter((folder) => !folder.is_hidden)
  const childrenByParent = new Map()

  for (const folder of visibleFolders) {
    const parentId = folder.parent_folder_id || '__root__'
    const bucket = childrenByParent.get(parentId) || []
    bucket.push(folder)
    childrenByParent.set(parentId, bucket)
  }

  for (const children of childrenByParent.values()) {
    children.sort((left, right) => left.name.localeCompare(right.name))
  }

  const topLevelFolders = childrenByParent.get(rootFolder?.id || '__root__') || []

  return {
    rootFolder,
    visibleFolders,
    childrenByParent,
    topLevelFolders,
  }
}

function WorkspaceApp({ routePath }) {
  const currentProjectId = parseProjectId(routePath)
  const fileInputRef = useRef(null)
  const [theme, setTheme] = useState(() => window.localStorage.getItem('menthtex-theme') || 'dark')
  const [flash, setFlash] = useState(null)
  const [session, setSession] = useState({
    loading: true,
    enabled: false,
    authenticated: false,
    providers: [],
    user: null,
  })
  const [projects, setProjects] = useState([])
  const [projectsLoading, setProjectsLoading] = useState(false)
  const [projectTree, setProjectTree] = useState(null)
  const [projectTags, setProjectTags] = useState([])
  const [projectMembers, setProjectMembers] = useState([])
  const [treeLoading, setTreeLoading] = useState(false)
  const [selectedFolderId, setSelectedFolderId] = useState(undefined)
  const [selectedDocumentId, setSelectedDocumentId] = useState(null)
  const [documentDetail, setDocumentDetail] = useState(null)
  const [detailEditing, setDetailEditing] = useState(false)
  const [documentForm, setDocumentForm] = useState({
    title: '',
    authors_text: '',
    year: '',
    doi: '',
    abstract_text: '',
    journal: '',
    booktitle: '',
    folder_id: '',
  })
  const [searchTerm, setSearchTerm] = useState('')
  const [selectedTag, setSelectedTag] = useState('')
  const [expandedFolders, setExpandedFolders] = useState({})
  const [createProjectName, setCreateProjectName] = useState('')
  const [createProjectDescription, setCreateProjectDescription] = useState('')
  const [createProjectVisibility, setCreateProjectVisibility] = useState('private')
  const [newFolderOpen, setNewFolderOpen] = useState(false)
  const [newFolderName, setNewFolderName] = useState('')
  const [newFolderParentId, setNewFolderParentId] = useState('')
  const [shareModalOpen, setShareModalOpen] = useState(false)
  const [shareSearch, setShareSearch] = useState('')
  const [shareResults, setShareResults] = useState([])
  const [shareRole, setShareRole] = useState('viewer')
  const [shareLoading, setShareLoading] = useState(false)
  const [definitionOptions, setDefinitionOptions] = useState([])
  const [definitionOptionsLoading, setDefinitionOptionsLoading] = useState(false)
  const [definitionOptionsError, setDefinitionOptionsError] = useState('')
  const [definitionOptionQuery, setDefinitionOptionQuery] = useState('')
  const [activeVenueChooserField, setActiveVenueChooserField] = useState('')
  const [tagDraft, setTagDraft] = useState('')
  const [noteDraft, setNoteDraft] = useState('')
  const [noteEdits, setNoteEdits] = useState({})
  const [draggingDocumentId, setDraggingDocumentId] = useState(null)
  const [dragTargetFolderId, setDragTargetFolderId] = useState(null)
  const [uploadDropActive, setUploadDropActive] = useState(false)
  const [processingTask, setProcessingTask] = useState(null)
  const [processingTick, setProcessingTick] = useState(() => Date.now())
  const [busyAction, setBusyAction] = useState('')
  const [versionInfo, setVersionInfo] = useState({
    appVersion: FRONTEND_VERSION,
    backendVersion: '',
  })

  useEffect(() => {
    document.documentElement.dataset.theme = theme
    window.localStorage.setItem('menthtex-theme', theme)
  }, [theme])

  useEffect(() => {
    if (!processingTask) {
      return undefined
    }

    setProcessingTick(Date.now())
    const intervalId = window.setInterval(() => {
      setProcessingTick(Date.now())
    }, 400)

    return () => {
      window.clearInterval(intervalId)
    }
  }, [processingTask])

  useEffect(() => {
    if (!flash) {
      return undefined
    }

    const timeoutMs = flash.type === 'error' ? 6500 : 3600
    const timeoutId = window.setTimeout(() => {
      setFlash(null)
    }, timeoutMs)

    return () => {
      window.clearTimeout(timeoutId)
    }
  }, [flash])

  useEffect(() => {
    let cancelled = false

    async function loadVersionInfo() {
      try {
        const payload = await requestJson('api/health')
        if (!cancelled) {
          setVersionInfo({
            appVersion: payload.app_version || FRONTEND_VERSION,
            backendVersion: payload.backend_version || '',
          })
        }
      } catch {
        if (!cancelled) {
          setVersionInfo({
            appVersion: FRONTEND_VERSION,
            backendVersion: '',
          })
        }
      }
    }

    loadVersionInfo()
    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    let cancelled = false

    async function loadSession() {
      try {
        const payload = await requestJson('api/me')
        if (!cancelled) {
          setSession({
            loading: false,
            enabled: payload.enabled,
            authenticated: payload.authenticated,
            providers: Array.isArray(payload.providers) ? payload.providers : [],
            user: payload.user || null,
          })
        }
      } catch (error) {
        if (!cancelled) {
          setSession({
            loading: false,
            enabled: true,
            authenticated: false,
            providers: [],
            user: null,
          })
          setFlash({ type: 'error', message: error.message })
        }
      }
    }

    loadSession()
    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    if (!session.authenticated) {
      setProjects([])
      return
    }

    let cancelled = false
    setProjectsLoading(true)

    requestJson('api/projects')
      .then((payload) => {
        if (!cancelled) {
          setProjects(payload.projects || [])
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setFlash({ type: 'error', message: error.message })
        }
      })
      .finally(() => {
        if (!cancelled) {
          setProjectsLoading(false)
        }
      })

    return () => {
      cancelled = true
    }
  }, [session.authenticated])

  useEffect(() => {
    setSelectedFolderId(undefined)
    setSelectedDocumentId(null)
    setDocumentDetail(null)
    setProjectTree(null)
    setProjectTags([])
    setProjectMembers([])
    setNewFolderOpen(false)
    setNewFolderName('')
    setNewFolderParentId('')
    setSearchTerm('')
    setSelectedTag('')
    setExpandedFolders({})
    setTagDraft('')
    setNoteDraft('')
    setNoteEdits({})

    if (!currentProjectId || !session.authenticated) {
      return
    }

    let cancelled = false
    setTreeLoading(true)

    Promise.all([
      requestJson(`api/projects/${currentProjectId}/tree`),
      requestJson(`api/projects/${currentProjectId}/tags`),
    ])
      .then(([treePayload, tagsPayload]) => {
        if (cancelled) {
          return
        }
        setProjectTree(treePayload)
        setProjectTags(tagsPayload.tags || [])
      })
      .catch((error) => {
        if (!cancelled) {
          setFlash({ type: 'error', message: error.message })
        }
      })
      .finally(() => {
        if (!cancelled) {
          setTreeLoading(false)
        }
      })

    return () => {
      cancelled = true
    }
  }, [currentProjectId, session.authenticated])

  useEffect(() => {
    if (!projectTree) {
      return
    }

    setExpandedFolders((current) => {
      const next = { ...current }
      for (const folder of projectTree.folders || []) {
        if (!folder.is_hidden && next[folder.id] === undefined) {
          next[folder.id] = true
        }
      }
      return next
    })
  }, [projectTree])

  useEffect(() => {
    if (!projectTree || selectedFolderId !== undefined) {
      return
    }
    setSelectedFolderId(null)
  }, [projectTree, selectedFolderId])

  const folderTree = useMemo(() => createFolderTree(projectTree?.folders || []), [projectTree])
  const activeProcessingMessage = processingPhase(processingTask, processingTick)

  const filteredDocuments = useMemo(() => {
    const documents = projectTree?.documents || []
    const normalizedSearch = searchTerm.trim().toLowerCase()
    const normalizedTag = selectedTag.trim().toLowerCase()

    return documents.filter((document) => {
      if (selectedFolderId && document.folder_id !== selectedFolderId) {
        return false
      }
      if (!matchesDocumentSearch(document, normalizedSearch)) {
        return false
      }
      if (
        normalizedTag &&
        !(document.tags || []).some((tag) => tag.name.toLowerCase() === normalizedTag)
      ) {
        return false
      }
      return true
    })
  }, [projectTree, searchTerm, selectedTag, selectedFolderId])

  useEffect(() => {
    if (!filteredDocuments.length) {
      setSelectedDocumentId(null)
      return
    }

    if (selectedDocumentId && filteredDocuments.some((document) => document.id === selectedDocumentId)) {
      return
    }

    setSelectedDocumentId(filteredDocuments[0].id)
  }, [filteredDocuments, selectedDocumentId])

  useEffect(() => {
    if (!selectedDocumentId || !session.authenticated) {
      setDocumentDetail(null)
      return
    }

    let cancelled = false

    requestJson(`api/documents/${selectedDocumentId}`)
      .then((payload) => {
        if (!cancelled) {
          setDocumentDetail(payload)
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setFlash({ type: 'error', message: error.message })
        }
      })

    return () => {
      cancelled = true
    }
  }, [selectedDocumentId, session.authenticated])

  useEffect(() => {
    setDocumentForm({
      title: documentDetail?.title || '',
      authors_text: documentDetail?.authors_text || '',
      year: documentDetail?.year || '',
      doi: documentDetail?.doi || '',
      abstract_text: documentDetail?.abstract_text || '',
      journal: documentDetail?.journal || '',
      booktitle: documentDetail?.booktitle || '',
      folder_id: documentDetail?.folder_id || '',
    })
    setNoteEdits({})
    setActiveVenueChooserField('')
    setDefinitionOptionQuery('')
    setDetailEditing(false)
  }, [documentDetail])

  useEffect(() => {
    if (!shareModalOpen || !currentProjectId) {
      return
    }

    let cancelled = false
    setShareLoading(true)

    requestJson(`api/projects/${currentProjectId}/members`)
      .then((payload) => {
        if (!cancelled) {
          setProjectMembers(payload.members || [])
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setFlash({ type: 'error', message: error.message })
        }
      })
      .finally(() => {
        if (!cancelled) {
          setShareLoading(false)
        }
      })

    return () => {
      cancelled = true
    }
  }, [shareModalOpen, currentProjectId])

  useEffect(() => {
    if (!shareModalOpen || !shareSearch.trim()) {
      setShareResults([])
      return
    }

    let cancelled = false
    const timeoutId = window.setTimeout(() => {
      requestJson(`api/users?query=${encodeURIComponent(shareSearch.trim())}`)
        .then((payload) => {
          if (cancelled) {
            return
          }
          const existingIds = new Set(projectMembers.map((member) => member.user_id))
          setShareResults((payload.users || []).filter((user) => !existingIds.has(user.id)))
        })
        .catch((error) => {
          if (!cancelled) {
            setFlash({ type: 'error', message: error.message })
          }
        })
    }, 220)

    return () => {
      cancelled = true
      window.clearTimeout(timeoutId)
    }
  }, [shareModalOpen, shareSearch, projectMembers])

  const projectRole = projectTree?.project?.role || null
  const editingAllowed = canEdit(projectRole)
  const ownerAllowed = isOwner(projectRole)
  const workspaceFooter = (
    <footer className="app-footer">
      <p className="app-version-line">
        MenthBibTex Formatter v{versionInfo.appVersion}
        {' · '}
        Frontend v{FRONTEND_VERSION}
        {versionInfo.backendVersion ? ` · Backend v${versionInfo.backendVersion}` : ''}
      </p>
      <p className="app-footer-note">
        Made with love, BibTeX, and questionable vibe coding by Fabian and Claude.
      </p>
      <p className="app-footer-link">
        <a
          href="https://github.com/n1tr0-5urf3r/menthTex"
          target="_blank"
          rel="noreferrer noopener"
        >
          Source on GitHub
        </a>
      </p>
    </footer>
  )

  const selectedFolder = folderTree.visibleFolders.find((folder) => folder.id === selectedFolderId) || null

  async function refreshProjectTree() {
    if (!currentProjectId) {
      return
    }

    const [treePayload, tagsPayload] = await Promise.all([
      requestJson(`api/projects/${currentProjectId}/tree`),
      requestJson(`api/projects/${currentProjectId}/tags`),
    ])

    setProjectTree(treePayload)
    setProjectTags(tagsPayload.tags || [])
  }

  async function refreshProjects() {
    if (!session.authenticated) {
      return
    }
    const payload = await requestJson('api/projects')
    setProjects(payload.projects || [])
  }

  function openLogin(providerId) {
    const target = new URL(apiPath(`api/auth/login/${providerId}`), window.location.origin)
    const currentUrl = new URL(window.location.href)
    currentUrl.searchParams.delete('auth')
    currentUrl.searchParams.delete('auth_error')
    target.searchParams.set(
      'return_to',
      `${currentUrl.pathname}${currentUrl.search}${currentUrl.hash}` || withBasePath('app'),
    )
    window.location.href = target.toString()
  }

  async function handleLogout() {
    try {
      await requestJson('api/auth/logout', { method: 'POST' })
      setSession({
        loading: false,
        enabled: true,
        authenticated: false,
        providers: session.providers,
        user: null,
      })
      navigateTo('/')
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    }
  }

  async function handleCreateProject(event) {
    event.preventDefault()
    try {
      setBusyAction('create-project')
      const project = await requestJson('api/projects', {
        method: 'POST',
        body: JSON.stringify({
          name: createProjectName,
          description: createProjectDescription,
          visibility: createProjectVisibility,
        }),
      })
      setCreateProjectName('')
      setCreateProjectDescription('')
      setCreateProjectVisibility('private')
      await refreshProjects()
      navigateTo(`/app/projects/${project.id}`)
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    } finally {
      setBusyAction('')
    }
  }

  async function handleCreateFolder(event) {
    event.preventDefault()
    if (!currentProjectId) {
      return
    }

    try {
      setBusyAction('create-folder')
      const created = await requestJson(`api/projects/${currentProjectId}/folders`, {
        method: 'POST',
        body: JSON.stringify({
          name: newFolderName,
          parent_folder_id: newFolderParentId || selectedFolderId || undefined,
        }),
      })
      await refreshProjectTree()
      setNewFolderName('')
      setNewFolderParentId('')
      setNewFolderOpen(false)
      setExpandedFolders((current) => ({ ...current, [created.id]: true }))
      setSelectedFolderId(created.id)
      setFlash({ type: 'success', message: `Folder "${created.name}" created.` })
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    } finally {
      setBusyAction('')
    }
  }

  async function handleDeleteFolder(folder) {
    const confirmed = window.confirm(`Delete folder "${folder.name}"? Empty folders only can be deleted.`)
    if (!confirmed) {
      return
    }

    try {
      setBusyAction(`delete-folder-${folder.id}`)
      await request(`api/folders/${folder.id}`, { method: 'DELETE' })
      if (selectedFolderId === folder.id) {
        setSelectedFolderId(null)
      }
      await refreshProjectTree()
      setFlash({ type: 'success', message: `Folder "${folder.name}" deleted.` })
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    } finally {
      setBusyAction('')
    }
  }

  async function handleUploadFiles(fileList) {
    if (!currentProjectId || !fileList?.length) {
      return
    }

    const pdfFiles = Array.from(fileList).filter(
      (file) => file.type === 'application/pdf' || file.name.toLowerCase().endsWith('.pdf'),
    )
    if (!pdfFiles.length) {
      setFlash({ type: 'error', message: 'Please upload PDF files only.' })
      return
    }

    try {
      setBusyAction('upload-documents')
      let lastUploaded = null

      for (const [index, file] of pdfFiles.entries()) {
        setProcessingTask({
          kind: 'upload',
          startedAt: Date.now(),
          fileName: file.name,
          index: index + 1,
          total: pdfFiles.length,
        })
        const formData = new FormData()
        formData.append('file', file)
        if (selectedFolderId) {
          formData.append('folder_id', selectedFolderId)
        }
        lastUploaded = await requestJson(`api/projects/${currentProjectId}/documents`, {
          method: 'POST',
          body: formData,
        })
      }

      await refreshProjectTree()
      await refreshProjects()
      if (lastUploaded?.id) {
        setSelectedDocumentId(lastUploaded.id)
      }
      setFlash({
        type: 'success',
        message:
          pdfFiles.length === 1
            ? `Uploaded "${pdfFiles[0].name}".`
            : `Uploaded ${pdfFiles.length} PDF files.`,
      })
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    } finally {
      setBusyAction('')
      setProcessingTask(null)
      setUploadDropActive(false)
      if (fileInputRef.current) {
        fileInputRef.current.value = ''
      }
    }
  }

  async function handleMoveDocument(documentId, folderId) {
    if (!documentId || !folderId) {
      return
    }

    const currentDocument = (projectTree?.documents || []).find((document) => document.id === documentId)
    if (currentDocument?.folder_id === folderId) {
      return
    }

    try {
      setBusyAction(`move-document-${documentId}`)
      const detail =
        documentDetail?.id === documentId ? documentDetail : await requestJson(`api/documents/${documentId}`)
      const payload = await requestJson(`api/documents/${documentId}`, {
        method: 'PATCH',
        body: JSON.stringify({
          folder_id: folderId,
          title: detail.title || '',
          authors_text: detail.authors_text || '',
          year: detail.year || '',
          doi: detail.doi || '',
          abstract_text: detail.abstract_text || '',
        }),
      })
      setDocumentDetail((current) => (current?.id === payload.id ? payload : current))
      await refreshProjectTree()
      await refreshProjects()
      setFlash({ type: 'success', message: 'Document moved.' })
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    } finally {
      setBusyAction('')
      setDragTargetFolderId(null)
      setDraggingDocumentId(null)
    }
  }

  async function persistDocumentMetadata(overrides = {}) {
    if (!documentDetail) {
      return null
    }

    const nextForm = {
      ...documentForm,
      ...overrides,
    }

      const payload = await requestJson(`api/documents/${documentDetail.id}`, {
        method: 'PATCH',
        body: JSON.stringify({
        title: nextForm.title,
        authors_text: nextForm.authors_text,
        year: nextForm.year,
        doi: nextForm.doi,
        abstract_text: nextForm.abstract_text,
        journal: nextForm.journal,
        booktitle: nextForm.booktitle,
        folder_id: nextForm.folder_id || undefined,
      }),
      })
      setDocumentDetail(payload)
      await refreshProjectTree()
      await refreshProjects()
      return payload
  }

  async function handleSaveMetadata(event) {
    event.preventDefault()
    if (!documentDetail) {
      return
    }

    try {
      setBusyAction('save-document')
      await persistDocumentMetadata()
      setDetailEditing(false)
      setFlash({ type: 'success', message: 'Document metadata saved.' })
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    } finally {
      setBusyAction('')
    }
  }

  function handleCancelMetadataEdit() {
    setDocumentForm({
      title: documentDetail?.title || '',
      authors_text: documentDetail?.authors_text || '',
      year: documentDetail?.year || '',
      doi: documentDetail?.doi || '',
      abstract_text: documentDetail?.abstract_text || '',
      journal: documentDetail?.journal || '',
      booktitle: documentDetail?.booktitle || '',
      folder_id: documentDetail?.folder_id || '',
    })
    setDetailEditing(false)
  }

  async function loadDefinitionOptions() {
    setDefinitionOptionsLoading(true)
    setDefinitionOptionsError('')
    try {
      const payload = await requestJson('api/definition-options')
      setDefinitionOptions(payload.definitions || [])
    } catch (error) {
      setDefinitionOptionsError(error.message)
    } finally {
      setDefinitionOptionsLoading(false)
    }
  }

  async function handleToggleVenueChooser(fieldName) {
    if (activeVenueChooserField === fieldName) {
      setActiveVenueChooserField('')
      setDefinitionOptionQuery('')
      setDefinitionOptionsError('')
      return
    }

    setActiveVenueChooserField(fieldName)
    setDefinitionOptionQuery('')
    if (definitionOptions.length === 0 && !definitionOptionsLoading) {
      await loadDefinitionOptions()
    }
  }

  async function handleUseExistingVenue(fieldName, token, successMessage = 'Venue updated.') {
    try {
      setBusyAction('save-document')
      await persistDocumentMetadata({ [fieldName]: token })
      setFlash({ type: 'success', message: successMessage })
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    } finally {
      setBusyAction('')
    }
  }

  async function handleAddVenueDefinition(review) {
    const proposal = review?.proposed_definition
    if (!proposal) {
      return
    }

    try {
      setBusyAction(`add-definition-${review.field_name}`)
      await requestJson('api/add-definition', {
        method: 'POST',
        body: JSON.stringify({
          token: proposal.token,
          raw_value: proposal.raw_value,
        }),
      })
      await handleUseExistingVenue(
        review.field_name,
        proposal.token,
        `Added ${proposal.token} and updated ${venueFieldLabel(review.field_name).toLowerCase()}.`,
      )
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    } finally {
      setBusyAction('')
    }
  }

  async function handleReprocessDocument() {
    if (!documentDetail) {
      return
    }

    try {
      setBusyAction('reprocess-document')
      setProcessingTask({
        kind: 'reprocess',
        startedAt: Date.now(),
        fileName: documentDetail.original_filename,
        documentId: documentDetail.id,
      })
      const payload = await requestJson(`api/documents/${documentDetail.id}/reprocess`, {
        method: 'POST',
      })
      setDocumentDetail(payload)
      await refreshProjectTree()
      setFlash({ type: 'success', message: 'Document metadata reprocessed.' })
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    } finally {
      setBusyAction('')
      setProcessingTask(null)
    }
  }

  async function handleDeleteDocument() {
    if (!documentDetail) {
      return
    }

    const confirmed = window.confirm(`Delete "${documentDetail.original_filename}"?`)
    if (!confirmed) {
      return
    }

    try {
      setBusyAction('delete-document')
      await request(`api/documents/${documentDetail.id}`, { method: 'DELETE' })
      setDocumentDetail(null)
      setSelectedDocumentId(null)
      await refreshProjectTree()
      await refreshProjects()
      setFlash({ type: 'success', message: 'Document deleted.' })
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    } finally {
      setBusyAction('')
    }
  }

  async function handleAddTag(event) {
    event.preventDefault()
    if (!documentDetail || !tagDraft.trim()) {
      return
    }

    try {
      setBusyAction('add-tag')
      const tags = await requestJson(`api/documents/${documentDetail.id}/tags`, {
        method: 'POST',
        body: JSON.stringify({ tags: [tagDraft] }),
      })
      setDocumentDetail((current) => (current ? { ...current, tags } : current))
      setTagDraft('')
      await refreshProjectTree()
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    } finally {
      setBusyAction('')
    }
  }

  async function handleRemoveTag(tagId) {
    if (!documentDetail) {
      return
    }

    try {
      await request(`api/documents/${documentDetail.id}/tags/${tagId}`, { method: 'DELETE' })
      const refreshed = await requestJson(`api/documents/${documentDetail.id}`)
      setDocumentDetail(refreshed)
      await refreshProjectTree()
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    }
  }

  async function handleCreateNote(event) {
    event.preventDefault()
    if (!documentDetail || !noteDraft.trim()) {
      return
    }

    try {
      setBusyAction('create-note')
      const note = await requestJson(`api/documents/${documentDetail.id}/notes`, {
        method: 'POST',
        body: JSON.stringify({ content: noteDraft }),
      })
      setDocumentDetail((current) =>
        current ? { ...current, notes: [note, ...(current.notes || [])] } : current,
      )
      setNoteDraft('')
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    } finally {
      setBusyAction('')
    }
  }

  async function handleSaveNote(noteId) {
    const content = noteEdits[noteId]
    if (!content?.trim()) {
      return
    }

    try {
      const note = await requestJson(`api/notes/${noteId}`, {
        method: 'PATCH',
        body: JSON.stringify({ content }),
      })
      setDocumentDetail((current) =>
        current
          ? {
              ...current,
              notes: (current.notes || []).map((existing) => (existing.id === note.id ? note : existing)),
            }
          : current,
      )
      setNoteEdits((current) => {
        const next = { ...current }
        delete next[noteId]
        return next
      })
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    }
  }

  async function handleDeleteNote(noteId) {
    try {
      await request(`api/notes/${noteId}`, { method: 'DELETE' })
      setDocumentDetail((current) =>
        current
          ? {
              ...current,
              notes: (current.notes || []).filter((note) => note.id !== noteId),
            }
          : current,
      )
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    }
  }

  async function handleExportBibtex() {
    if (!currentProjectId) {
      return
    }

    try {
      const text = await requestText(`api/projects/${currentProjectId}/export/bibtex`)
      triggerDownload('literature.bib', text, 'application/x-bibtex;charset=utf-8')
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    }
  }

  async function handleDeleteProject() {
    if (!currentProjectId || !projectTree?.project) {
      return
    }

    const confirmed = window.confirm(`Delete project "${projectTree.project.name}"?`)
    if (!confirmed) {
      return
    }

    try {
      setBusyAction('delete-project')
      await request(`api/projects/${currentProjectId}`, { method: 'DELETE' })
      await refreshProjects()
      navigateTo('/app')
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    } finally {
      setBusyAction('')
    }
  }

  async function handleToggleProjectVisibility() {
    if (!currentProjectId || !projectTree?.project) {
      return
    }

    const nextVisibility = projectTree.project.visibility === 'public' ? 'private' : 'public'
    const confirmed = window.confirm(`Do you want to make this project ${nextVisibility}?`)
    if (!confirmed) {
      return
    }

    try {
      setBusyAction('update-project-visibility')
      const updated = await requestJson(`api/projects/${currentProjectId}`, {
        method: 'PATCH',
        body: JSON.stringify({ visibility: nextVisibility }),
      })
      setProjectTree((current) => (current ? { ...current, project: updated } : current))
      await refreshProjects()
      setFlash({ type: 'success', message: `Project is now ${updated.visibility}.` })
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    } finally {
      setBusyAction('')
    }
  }

  async function handleAddMember(userId) {
    if (!currentProjectId) {
      return
    }

    try {
      await requestJson(`api/projects/${currentProjectId}/members`, {
        method: 'POST',
        body: JSON.stringify({
          user_id: userId,
          role: shareRole,
        }),
      })
      const payload = await requestJson(`api/projects/${currentProjectId}/members`)
      setProjectMembers(payload.members || [])
      setShareResults((current) => current.filter((user) => user.id !== userId))
      setShareSearch('')
      setFlash({ type: 'success', message: 'Member added.' })
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    }
  }

  async function handleUpdateMemberRole(userId, role) {
    if (!currentProjectId) {
      return
    }

    try {
      const member = await requestJson(`api/projects/${currentProjectId}/members/${userId}`, {
        method: 'PATCH',
        body: JSON.stringify({ role }),
      })
      setProjectMembers((current) =>
        current.map((existing) => (existing.user_id === userId ? member : existing)),
      )
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    }
  }

  async function handleRemoveMember(userId) {
    if (!currentProjectId) {
      return
    }

    try {
      await request(`api/projects/${currentProjectId}/members/${userId}`, { method: 'DELETE' })
      setProjectMembers((current) => current.filter((member) => member.user_id !== userId))
    } catch (error) {
      setFlash({ type: 'error', message: error.message })
    }
  }

  function renderFolderNode(folder, depth = 0) {
    const children = folderTree.childrenByParent.get(folder.id) || []
    const open = expandedFolders[folder.id] !== false
    const isActive = selectedFolderId === folder.id
    const isDropTarget = dragTargetFolderId === folder.id && draggingDocumentId

    return (
      <div className="workspace-folder-node" key={folder.id}>
        <div
          className={[
            'workspace-folder-row',
            isActive ? 'workspace-folder-row-active' : '',
            isDropTarget ? 'workspace-folder-row-drop' : '',
          ]
            .filter(Boolean)
            .join(' ')}
          style={{ '--folder-depth': depth }}
          onDragOver={(event) => {
            if (!draggingDocumentId) {
              return
            }
            event.preventDefault()
            setDragTargetFolderId(folder.id)
          }}
          onDragLeave={() => {
            if (dragTargetFolderId === folder.id) {
              setDragTargetFolderId(null)
            }
          }}
          onDrop={(event) => {
            event.preventDefault()
            handleMoveDocument(draggingDocumentId, folder.id)
          }}
        >
          <button
            type="button"
            className="workspace-folder-toggle"
            onClick={() => {
              if (!children.length) {
                return
              }
              setExpandedFolders((current) => ({
                ...current,
                [folder.id]: current[folder.id] === false,
              }))
            }}
            aria-label={children.length ? (open ? 'Collapse folder' : 'Expand folder') : 'Leaf folder'}
          >
            {children.length ? <ChevronIcon open={open} /> : null}
          </button>
          <button
            type="button"
            className="workspace-folder-select"
            onClick={() => setSelectedFolderId(folder.id)}
            title={folder.name}
          >
            <FolderIcon />
            <span>{folder.name}</span>
          </button>
          {editingAllowed ? (
            <div className="workspace-folder-actions">
              <button
                type="button"
                className="workspace-icon-button"
                onClick={() => {
                  setNewFolderOpen(true)
                  setNewFolderParentId(folder.id)
                }}
                title="Add subfolder"
                aria-label="Add subfolder"
              >
                <PlusIcon />
              </button>
              <button
                type="button"
                className="workspace-icon-button workspace-icon-danger"
                onClick={() => handleDeleteFolder(folder)}
                title="Delete folder"
                aria-label="Delete folder"
                disabled={busyAction === `delete-folder-${folder.id}`}
              >
                <TrashIcon />
              </button>
            </div>
          ) : null}
        </div>
        {children.length && open ? (
          <div className="workspace-folder-children">
            {children.map((child) => renderFolderNode(child, depth + 1))}
          </div>
        ) : null}
      </div>
    )
  }

  if (session.loading) {
    return (
      <main className="workspace-shell">
        <section className="workspace-hero">
          <div className="workspace-hero-copy">
            <p className="workspace-eyebrow">Library workspace</p>
            <h1>Loading workspace</h1>
            <p className="workspace-copy">Checking your authentication state and project access.</p>
          </div>
        </section>
        {workspaceFooter}
      </main>
    )
  }

  if (!session.enabled) {
    return (
      <main className="workspace-shell">
        <section className="workspace-hero">
          <div className="workspace-hero-copy">
            <p className="workspace-eyebrow">Library workspace</p>
            <h1>Workspace unavailable</h1>
            <p className="workspace-copy">
              Authentication is not configured, so only the public BibTeX formatter is currently available.
            </p>
          </div>
          <div className="workspace-toolbar">
            <button type="button" className="btn btn-outline-secondary btn-sm" onClick={() => navigateTo('/')}>
              Public formatter
            </button>
            <button
              type="button"
              className="theme-toggle"
              onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
              aria-label={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
            >
              {theme === 'dark' ? <SunIcon /> : <MoonIcon />}
            </button>
          </div>
        </section>
        {workspaceFooter}
      </main>
    )
  }

  if (!session.authenticated) {
    return (
      <main className="workspace-shell">
        <section className="workspace-hero">
          <div className="workspace-hero-copy">
            <p className="workspace-eyebrow">Library workspace</p>
            <h1>Sign in to access shared projects</h1>
            <p className="workspace-copy">
              The literature manager is restricted to authenticated users with kn-staff or kn-esslingen membership.
            </p>
          </div>
          <div className="workspace-toolbar">
            <button type="button" className="btn btn-outline-secondary btn-sm" onClick={() => navigateTo('/')}>
              Public formatter
            </button>
            <LoginProviderPopup
              providers={session.providers}
              onSelectProvider={openLogin}
              buttonClassName="btn btn-primary btn-sm"
            />
            <button
              type="button"
              className="theme-toggle"
              onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
              aria-label={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
            >
              {theme === 'dark' ? <SunIcon /> : <MoonIcon />}
            </button>
          </div>
        </section>
        {workspaceFooter}
      </main>
    )
  }

  return (
    <main className="workspace-shell">
      <section className="workspace-hero">
        <div className="workspace-hero-utility">
          {session.user ? (
            <span className="workspace-user-summary">{formatUserLabel(session.user)}</span>
          ) : null}
          <button
            type="button"
            className="workspace-icon-button"
            onClick={handleLogout}
            title="Sign out"
            aria-label="Sign out"
          >
            <LogoutIcon />
          </button>
          <button
            type="button"
            className="theme-toggle"
            onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
            aria-label={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
            title={theme === 'dark' ? 'Light mode' : 'Dark mode'}
          >
            {theme === 'dark' ? <SunIcon /> : <MoonIcon />}
          </button>
        </div>
        <div className="workspace-hero-main">
          <div className="workspace-brand-row">
            <img
              src={publicPath('logo_clean.png')}
              width="72"
              height="72"
              alt="menthTex logo"
              className="workspace-brand-logo"
            />
            <div className="workspace-brand-copy">
              <p className="workspace-eyebrow">Research chair library</p>
              <h1>{projectTree?.project?.name || 'Project library'}</h1>
              <p className="workspace-copy">
                Organize PDFs, notes, tags, and generated BibTeX entries inside shared project folders.
              </p>
            </div>
          </div>
          <div className="workspace-hero-nav" aria-label="Workspace navigation">
            <button
              type="button"
              className="btn btn-outline-secondary btn-sm"
              onClick={() => navigateTo('/app')}
            >
              Projects
            </button>
            <button
              type="button"
              className="btn btn-outline-secondary btn-sm"
              onClick={() => navigateTo('/')}
            >
              Public formatter
            </button>
          </div>
        </div>
      </section>

      {flash ? (
        <div
          className={`workspace-flash workspace-flash-${flash.type || 'info'}`}
          role={flash.type === 'error' ? 'alert' : 'status'}
          aria-live={flash.type === 'error' ? 'assertive' : 'polite'}
        >
          <span>{flash.message}</span>
          <button type="button" onClick={() => setFlash(null)} aria-label="Dismiss message">
            ×
          </button>
        </div>
      ) : null}

      {!currentProjectId ? (
        <>
          <section className="workspace-projects-panel">
            <div className="workspace-panel-header">
              <div>
                <p className="workspace-eyebrow">Projects</p>
                <h2>Your literature collections</h2>
                <p className="workspace-copy">
                  Create a project per chair topic, thesis, or paper stream and share it with colleagues.
                </p>
              </div>
            </div>

            <form className="workspace-project-create" onSubmit={handleCreateProject}>
              <input
                className="form-control"
                placeholder="New project name"
                value={createProjectName}
                onChange={(event) => setCreateProjectName(event.target.value)}
              />
              <input
                className="form-control"
                placeholder="Optional project description"
                value={createProjectDescription}
                onChange={(event) => setCreateProjectDescription(event.target.value)}
              />
              <select
                className="form-select"
                value={createProjectVisibility}
                onChange={(event) => setCreateProjectVisibility(event.target.value)}
              >
                <option value="private">Private</option>
                <option value="public">Public</option>
              </select>
              <button type="submit" className="btn btn-primary" disabled={busyAction === 'create-project'}>
                Create project
              </button>
            </form>

            {projectsLoading ? (
              <div className="workspace-empty-state">Loading projects…</div>
            ) : projects.length ? (
              <div className="workspace-project-grid">
                {projects.map((project) => (
                  <button
                    type="button"
                    key={project.id}
                    className="workspace-project-card"
                    onClick={() => navigateTo(`/app/projects/${project.id}`)}
                  >
                    <p className="workspace-card-meta">
                      {visibilityLabel(project.visibility)} · {roleLabel(project.role)}
                    </p>
                    <h3>{project.name}</h3>
                    <p className="workspace-card-copy">
                      {project.description || 'No description yet.'}
                    </p>
                    <div className="workspace-project-stats">
                      <span>{project.document_count} documents</span>
                      <span>{project.member_count} members</span>
                    </div>
                  </button>
                ))}
              </div>
            ) : (
              <div className="workspace-empty-state">
                No projects yet. Create the first shared literature workspace above.
              </div>
            )}
          </section>
        </>
      ) : (
        <section className="workspace-layout workspace-library-layout">
          <aside className="workspace-sidebar workspace-library-tree">
            <section className="workspace-panel-card workspace-tree-panel">
              <div className="workspace-panel-header">
                <div>
                  <p className="workspace-eyebrow">Folders</p>
                  <h2>Collections</h2>
                </div>
                {editingAllowed ? (
                  <button
                    type="button"
                    className="workspace-icon-button workspace-icon-button-solid"
                    onClick={() => {
                      setNewFolderOpen((current) => !current)
                      setNewFolderParentId(selectedFolderId || '')
                    }}
                    aria-label="Add folder"
                    title="Add folder"
                  >
                    <PlusIcon />
                  </button>
                ) : null}
              </div>

              {newFolderOpen ? (
                <form className="workspace-inline-form workspace-inline-form-card" onSubmit={handleCreateFolder}>
                  <input
                    className="form-control"
                    placeholder="Folder name"
                    value={newFolderName}
                    onChange={(event) => setNewFolderName(event.target.value)}
                  />
                  <select
                    className="form-select"
                    value={newFolderParentId}
                    onChange={(event) => setNewFolderParentId(event.target.value)}
                  >
                    <option value="">Top level</option>
                    {folderTree.visibleFolders.map((folder) => (
                      <option key={folder.id} value={folder.id}>
                        {folder.name}
                      </option>
                    ))}
                  </select>
                  <div className="workspace-inline-actions">
                    <button type="submit" className="btn btn-primary btn-sm" disabled={busyAction === 'create-folder'}>
                      Add
                    </button>
                    <button
                      type="button"
                      className="btn btn-outline-secondary btn-sm"
                      onClick={() => {
                        setNewFolderOpen(false)
                        setNewFolderName('')
                        setNewFolderParentId('')
                      }}
                    >
                      Cancel
                    </button>
                  </div>
                </form>
              ) : null}

              <div className="workspace-folder-tree">
                <button
                  type="button"
                  className={[
                    'workspace-folder-row',
                    'workspace-folder-row-all',
                    selectedFolderId === null ? 'workspace-folder-row-active' : '',
                  ]
                    .filter(Boolean)
                    .join(' ')}
                  onClick={() => setSelectedFolderId(null)}
                >
                  <div className="workspace-folder-row-all-copy">
                    <span>All folders</span>
                    <small>{projectTree?.documents?.length || 0}</small>
                  </div>
                </button>

                {folderTree.topLevelFolders.length ? (
                  folderTree.topLevelFolders.map((folder) => renderFolderNode(folder, 0))
                ) : treeLoading ? (
                  <div className="workspace-empty-state">Loading folders…</div>
                ) : (
                  <div className="workspace-empty-state">No folders yet.</div>
                )}
              </div>
            </section>
          </aside>

          <section className="workspace-main-pane">
            <section className="workspace-panel-card workspace-project-strip">
              <div className="workspace-project-strip-top">
                <div className="workspace-project-strip-copy">
                  <strong>{projectTree?.project?.name || 'Loading project…'}</strong>
                  <span>{projectTree?.project?.description || 'Shared literature workspace'}</span>
                </div>
                <div className="workspace-project-strip-actions">
                  <span className="workspace-role-chip workspace-project-role-chip">
                    {roleLabel(projectRole)}
                  </span>
                  {ownerAllowed ? (
                    <button
                      type="button"
                      className="workspace-visibility-toggle"
                      onClick={handleToggleProjectVisibility}
                      disabled={busyAction === 'update-project-visibility'}
                      title={`Make project ${
                        projectTree?.project?.visibility === 'public' ? 'private' : 'public'
                      }`}
                    >
                      {projectTree?.project?.visibility === 'public' ? <GlobeIcon /> : <LockIcon />}
                      <span>{visibilityLabel(projectTree?.project?.visibility)}</span>
                    </button>
                  ) : (
                    <span className="workspace-role-chip">{visibilityLabel(projectTree?.project?.visibility)}</span>
                  )}
                  {ownerAllowed ? (
                    <button
                      type="button"
                      className="workspace-icon-button"
                      onClick={() => setShareModalOpen(true)}
                      title="Share project"
                      aria-label="Share project"
                    >
                      <ShareIcon />
                    </button>
                  ) : null}
                  <button
                    type="button"
                    className="workspace-icon-button"
                    onClick={handleExportBibtex}
                    title="Export BibTeX"
                    aria-label="Export BibTeX"
                  >
                    <DownloadIcon />
                  </button>
                  {ownerAllowed ? (
                    <button
                      type="button"
                      className="workspace-icon-button workspace-icon-danger"
                      onClick={handleDeleteProject}
                      title="Delete project"
                      aria-label="Delete project"
                      disabled={busyAction === 'delete-project'}
                    >
                      <TrashIcon />
                    </button>
                  ) : null}
                </div>
              </div>

            </section>

            <section className="workspace-panel-card workspace-document-toolbar">
              <div className="workspace-document-toolbar-context">
                <span className="workspace-document-summary-label">Browsing</span>
                <strong>{selectedFolderId ? folderDisplayName(selectedFolder) : 'All folders'}</strong>
                <span className="workspace-document-summary-divider" aria-hidden="true">
                  ·
                </span>
                <span className="workspace-document-summary-meta">
                  {filteredDocuments.length} document{filteredDocuments.length === 1 ? '' : 's'} visible
                </span>
              </div>
              <div className="workspace-document-toolbar-controls">
                <input
                  className="form-control"
                  placeholder="Search title, authors, DOI, key"
                  value={searchTerm}
                  onChange={(event) => setSearchTerm(event.target.value)}
                />
                <select
                  className="form-select"
                  value={selectedTag}
                  onChange={(event) => setSelectedTag(event.target.value)}
                >
                  <option value="">All tags</option>
                  {projectTags.map((tag) => (
                    <option key={tag.id} value={tag.name}>
                      {tag.name} ({tag.usage_count})
                    </option>
                  ))}
                </select>
                {editingAllowed ? (
                  <div className="workspace-document-controls">
                    <div
                      className={[
                        'workspace-upload-dropzone',
                        'workspace-upload-dropzone-toolbar',
                        uploadDropActive ? 'workspace-upload-dropzone-active' : '',
                      ]
                        .filter(Boolean)
                        .join(' ')}
                      onDragOver={(event) => {
                        event.preventDefault()
                        setUploadDropActive(true)
                      }}
                      onDragLeave={() => setUploadDropActive(false)}
                      onDrop={(event) => {
                        event.preventDefault()
                        setUploadDropActive(false)
                        handleUploadFiles(event.dataTransfer.files)
                      }}
                      onClick={() => fileInputRef.current?.click()}
                      role="button"
                      tabIndex={0}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter' || event.key === ' ') {
                          event.preventDefault()
                          fileInputRef.current?.click()
                        }
                      }}
                    >
                      <strong>Upload PDFs</strong>
                      <p>
                        Drop or click to upload into{' '}
                        {selectedFolderId
                          ? folderDisplayName(
                              folderTree.visibleFolders.find((folder) => folder.id === selectedFolderId),
                            )
                          : 'All folders'}
                        .
                      </p>
                      <input
                        ref={fileInputRef}
                        type="file"
                        accept="application/pdf,.pdf"
                        multiple
                        hidden
                        onChange={(event) => handleUploadFiles(event.target.files)}
                      />
                    </div>
                  </div>
                ) : null}
              </div>
              {processingTask ? (
                <div className="workspace-processing-banner" aria-live="polite">
                  <div className="workspace-processing-banner-main">
                    <span className="workspace-processing-spinner" aria-hidden="true">
                      <SpinnerIcon />
                    </span>
                    <div>
                      <strong>
                        {processingTask.kind === 'upload'
                          ? `Processing upload${processingTask.total > 1 ? ` ${processingTask.index}/${processingTask.total}` : ''}`
                          : 'Reprocessing document'}
                      </strong>
                      <p>{activeProcessingMessage}</p>
                    </div>
                  </div>
                  {processingTask.fileName ? (
                    <span className="workspace-processing-file">{processingTask.fileName}</span>
                  ) : null}
                </div>
              ) : null}
            </section>

            <section className="workspace-content workspace-library-content">
              <section className="workspace-panel-card workspace-document-results-panel">
                <div className="workspace-document-results-toolbar">
                  <div>
                    <p className="workspace-eyebrow">Documents</p>
                    <h2>Library items</h2>
                  </div>
                  <p className="workspace-copy">
                    Select a row to inspect or edit metadata.
                  </p>
                </div>
                <div className="workspace-document-table" role="table" aria-label="Project documents">
                  <div className="workspace-document-table-header" role="row">
                    <span role="columnheader">Title</span>
                    <span role="columnheader">Authors</span>
                    <span role="columnheader">Year</span>
                  </div>
                  <div className="workspace-document-list" role="rowgroup">
                  {filteredDocuments.length ? (
                    filteredDocuments.map((document) => (
                      <button
                        type="button"
                        key={document.id}
                        className={[
                          'workspace-document-row',
                          'workspace-document-table-row',
                          selectedDocumentId === document.id ? 'workspace-document-row-active' : '',
                        ]
                          .filter(Boolean)
                          .join(' ')}
                        onClick={() => setSelectedDocumentId(document.id)}
                        draggable={editingAllowed}
                        onDragStart={() => setDraggingDocumentId(document.id)}
                        onDragEnd={() => {
                          setDraggingDocumentId(null)
                          setDragTargetFolderId(null)
                        }}
                        role="row"
                      >
                        <span className="workspace-document-table-cell workspace-document-table-title" role="cell">
                          <strong>{document.title || document.original_filename}</strong>
                          {document.original_filename && document.title ? (
                            <small>{document.original_filename}</small>
                          ) : null}
                        </span>
                        <span className="workspace-document-table-cell" role="cell">
                          {document.authors_text || '—'}
                        </span>
                        <span className="workspace-document-table-cell workspace-document-year" role="cell">
                          {document.year || '—'}
                        </span>
                      </button>
                    ))
                  ) : (
                    <div className="workspace-empty-state">
                      No documents match the current folder, search, and tag filters.
                    </div>
                  )}
                  </div>
                </div>
              </section>

              <section className="workspace-panel-card workspace-detail-panel">
              {documentDetail ? (
                <>
                  <div className="workspace-panel-header">
                    <div>
                      <p className="workspace-eyebrow">Document detail</p>
                      <h2>{documentDetail.title || documentDetail.original_filename}</h2>
                      <p className="workspace-copy">{documentDetail.original_filename}</p>
                    </div>
                    <div className="workspace-detail-actions">
                      <a
                        className="workspace-icon-button"
                        href={apiPath(documentDetail.file_url.startsWith('/') ? documentDetail.file_url.slice(1) : documentDetail.file_url)}
                        target="_blank"
                        rel="noreferrer"
                        title="Open PDF"
                        aria-label="Open PDF"
                      >
                        <OpenFileIcon />
                      </a>
                      {editingAllowed ? (
                        <>
                          <button
                            type="button"
                            className={`workspace-icon-button ${detailEditing ? 'workspace-icon-button-active' : ''}`}
                            onClick={() => setDetailEditing(true)}
                            disabled={detailEditing}
                            title="Edit metadata"
                            aria-label="Edit metadata"
                          >
                            <PenIcon />
                          </button>
                          <button
                            type="button"
                            className={[
                              'workspace-icon-button',
                              busyAction === 'reprocess-document' ? 'workspace-icon-button-spinning' : '',
                            ]
                              .filter(Boolean)
                              .join(' ')}
                            onClick={handleReprocessDocument}
                            disabled={busyAction === 'reprocess-document'}
                            title="Reprocess document"
                            aria-label="Reprocess document"
                          >
                            {busyAction === 'reprocess-document' ? <SpinnerIcon /> : <RefreshIcon />}
                          </button>
                          <button
                            type="button"
                            className="workspace-icon-button workspace-icon-danger"
                            onClick={handleDeleteDocument}
                            disabled={busyAction === 'delete-document'}
                            title="Delete document"
                            aria-label="Delete document"
                          >
                            <TrashIcon />
                          </button>
                        </>
                      ) : null}
                    </div>
                  </div>

                  {documentDetail.duplicate_warnings?.length
                  || documentDetail.enrichment_warnings?.length
                  || (!documentDetail.journal?.trim() && !documentDetail.booktitle?.trim()) ? (
                    <div className="workspace-warning-list">
                      {documentDetail.duplicate_warnings.map((warning) => (
                        <div key={warning} className="workspace-warning-item">
                          {warning}
                        </div>
                      ))}
                      {(documentDetail.enrichment_warnings || []).map((warning) => (
                        <div key={warning} className="workspace-warning-item">
                          {warning}
                        </div>
                      ))}
                      {!documentDetail.journal?.trim() && !documentDetail.booktitle?.trim() ? (
                        <div className="workspace-warning-item">
                          Neither <code>journal</code> nor <code>booktitle</code> is set for this document.
                        </div>
                      ) : null}
                    </div>
                  ) : null}

                  <form
                    className={[
                      'workspace-detail-grid',
                      detailEditing ? 'workspace-detail-grid-editing' : 'workspace-detail-grid-readonly',
                    ]
                      .filter(Boolean)
                      .join(' ')}
                    onSubmit={handleSaveMetadata}
                  >
                    {detailEditing ? (
                      <>
                        <label>
                          <span>Title</span>
                          <input
                            className="form-control"
                            value={documentForm.title}
                            onChange={(event) =>
                              setDocumentForm((current) => ({ ...current, title: event.target.value }))
                            }
                          />
                        </label>
                        <label>
                          <span>Authors</span>
                          <input
                            className="form-control"
                            value={documentForm.authors_text}
                            onChange={(event) =>
                              setDocumentForm((current) => ({
                                ...current,
                                authors_text: event.target.value,
                              }))
                            }
                          />
                        </label>
                        <label>
                          <span>Year</span>
                          <input
                            className="form-control"
                            value={documentForm.year}
                            onChange={(event) =>
                              setDocumentForm((current) => ({ ...current, year: event.target.value }))
                            }
                          />
                        </label>
                        <label>
                          <span>DOI</span>
                          <input
                            className="form-control"
                            value={documentForm.doi}
                            onChange={(event) =>
                              setDocumentForm((current) => ({ ...current, doi: event.target.value }))
                            }
                          />
                        </label>
                        <label>
                          <span>Journal</span>
                          <input
                            className="form-control"
                            value={documentForm.journal}
                            onChange={(event) =>
                              setDocumentForm((current) => ({ ...current, journal: event.target.value }))
                            }
                          />
                        </label>
                        <label>
                          <span>Booktitle</span>
                          <input
                            className="form-control"
                            value={documentForm.booktitle}
                            onChange={(event) =>
                              setDocumentForm((current) => ({ ...current, booktitle: event.target.value }))
                            }
                          />
                        </label>
                        <label className="workspace-detail-textarea">
                          <span>Abstract / note</span>
                          <textarea
                            className="form-control"
                            value={documentForm.abstract_text}
                            onChange={(event) =>
                              setDocumentForm((current) => ({
                                ...current,
                                abstract_text: event.target.value,
                              }))
                            }
                          />
                        </label>
                      </>
                    ) : (
                      <>
                        <div className="workspace-detail-field">
                          <span>Title</span>
                          <p>{documentDetail.title || '—'}</p>
                        </div>
                        <div className="workspace-detail-field">
                          <span>Authors</span>
                          <p>{documentDetail.authors_text || '—'}</p>
                        </div>
                        <div className="workspace-detail-field">
                          <span>Year</span>
                          <p>{documentDetail.year || '—'}</p>
                        </div>
                        <div className="workspace-detail-field">
                          <span>DOI</span>
                          <p>{documentDetail.doi || '—'}</p>
                        </div>
                        <div className="workspace-detail-field">
                          <span>Journal</span>
                          <p>{documentDetail.journal || '—'}</p>
                        </div>
                        <div className="workspace-detail-field">
                          <span>Booktitle</span>
                          <p>{documentDetail.booktitle || '—'}</p>
                        </div>
                        <div className="workspace-detail-field workspace-detail-textarea">
                          <span>Abstract / note</span>
                          <p>{documentDetail.abstract_text || '—'}</p>
                        </div>
                      </>
                    )}

                    {editingAllowed && detailEditing ? (
                      <div className="workspace-detail-submit">
                        <button
                          type="button"
                          className="btn btn-outline-secondary btn-sm"
                          onClick={handleCancelMetadataEdit}
                        >
                          Cancel
                        </button>
                        <button
                          type="submit"
                          className="btn btn-primary btn-sm"
                          disabled={busyAction === 'save-document'}
                        >
                          Save metadata
                        </button>
                      </div>
                    ) : null}
                  </form>

                  {documentDetail.venue_reviews?.length ? (
                    <section className="workspace-venue-panel">
                      <div className="workspace-subsection-header">
                        <div>
                          <h3>Venue abbreviations</h3>
                          <p className="workspace-copy">
                            Reuse existing abbreviations or add a new one from this document.
                          </p>
                        </div>
                      </div>
                      <div className="workspace-venue-review-list">
                        {documentDetail.venue_reviews.map((review) => (
                          <article className="workspace-venue-card" key={review.field_name}>
                            <div className="workspace-venue-card-header">
                              <div>
                                <strong>{venueFieldLabel(review.field_name)}</strong>
                                <p className="workspace-copy">{review.value}</p>
                              </div>
                              {editingAllowed ? (
                                <button
                                  type="button"
                                  className={`btn btn-outline-secondary btn-sm icon-button ${
                                    activeVenueChooserField === review.field_name ? 'icon-button-active' : ''
                                  }`}
                                  onClick={() => handleToggleVenueChooser(review.field_name)}
                                  title="Choose existing abbreviation"
                                  aria-label={`Choose existing abbreviation for ${venueFieldLabel(review.field_name)}`}
                                >
                                  <BrowseIcon />
                                </button>
                              ) : null}
                            </div>

                            {review.matched_definition ? (
                              <div className="workspace-venue-resolution">
                                <span className="workspace-role-chip">{review.matched_definition.token}</span>
                                <span className="workspace-copy">
                                  Uses existing venue abbreviation.
                                </span>
                              </div>
                            ) : null}

                            {review.proposed_definition ? (
                              <div className="workspace-venue-proposal">
                                <div>
                                  <span className="field-label">Proposed abbreviation</span>
                                  <p className="mb-0">
                                    <strong>{review.proposed_definition.token}</strong>
                                    {' · '}
                                    {review.proposed_definition.expanded_value}
                                  </p>
                                </div>
                                {editingAllowed ? (
                                  <button
                                    type="button"
                                    className="btn btn-outline-secondary btn-sm"
                                    onClick={() => handleAddVenueDefinition(review)}
                                    disabled={busyAction === `add-definition-${review.field_name}`}
                                  >
                                    Add to conferences.bib
                                  </button>
                                ) : null}
                              </div>
                            ) : null}

                            {review.suggestions?.length ? (
                              <div className="definition-suggestion-list">
                                <span className="field-label">Similar existing venues</span>
                                {review.suggestions.map((suggestion) => (
                                  <div className="definition-suggestion" key={`${review.field_name}-${suggestion.token}`}>
                                    <div>
                                      <strong>{suggestion.token}</strong>
                                      <p className="helper-text">
                                        {suggestion.expanded_value}
                                        {typeof suggestion.score === 'number'
                                          ? ` · ${suggestion.score}% match`
                                          : ''}
                                      </p>
                                    </div>
                                    {editingAllowed ? (
                                      <button
                                        type="button"
                                        className="btn btn-outline-secondary btn-sm"
                                        onClick={() =>
                                          handleUseExistingVenue(
                                            review.field_name,
                                            suggestion.token,
                                            `${venueFieldLabel(review.field_name)} updated to ${suggestion.token}.`,
                                          )
                                        }
                                      >
                                        Use existing
                                      </button>
                                    ) : null}
                                  </div>
                                ))}
                              </div>
                            ) : null}

                            {activeVenueChooserField === review.field_name ? (
                              <div className="definition-chooser">
                                <span className="field-label">Choose an existing abbreviation</span>
                                <input
                                  type="text"
                                  className="form-control"
                                  value={definitionOptionQuery}
                                  onChange={(event) => setDefinitionOptionQuery(event.target.value)}
                                  placeholder="Filter by token or venue name"
                                />
                                {definitionOptionsLoading ? (
                                  <p className="helper-text mb-0">Loading existing abbreviations…</p>
                                ) : definitionOptionsError ? (
                                  <p className="definition-validation-error">{definitionOptionsError}</p>
                                ) : (
                                  (() => {
                                    const filteredOptions = definitionOptions
                                      .filter((option) => {
                                        const query = definitionOptionQuery.trim().toLowerCase()
                                        if (!query) {
                                          return true
                                        }
                                        return (
                                          option.token.toLowerCase().includes(query) ||
                                          option.expanded_value.toLowerCase().includes(query)
                                        )
                                      })
                                      .slice(0, 12)

                                    return filteredOptions.length ? (
                                      <div className="definition-chooser-list">
                                        {filteredOptions.map((option) => (
                                          <button
                                            type="button"
                                            className="definition-chooser-option"
                                            key={`${review.field_name}-${option.token}`}
                                            onClick={() =>
                                              handleUseExistingVenue(
                                                review.field_name,
                                                option.token,
                                                `${venueFieldLabel(review.field_name)} updated to ${option.token}.`,
                                              )
                                            }
                                          >
                                            <span className="definition-chooser-token">{option.token}</span>
                                            <span className="definition-chooser-value">{option.expanded_value}</span>
                                          </button>
                                        ))}
                                      </div>
                                    ) : (
                                      <p className="helper-text mb-0">No matching abbreviations found.</p>
                                    )
                                  })()
                                )}
                              </div>
                            ) : null}
                          </article>
                        ))}
                      </div>
                    </section>
                  ) : null}

                  <section className="workspace-tags-block">
                    <div className="workspace-subsection-header">
                      <div>
                        <h3>Tags</h3>
                        <p className="workspace-copy">Auto tags come from metadata and PDF text extraction.</p>
                      </div>
                    </div>
                    <div className="workspace-tag-list">
                      {(documentDetail.tags || []).map((tag) => (
                        <span
                          key={tag.id}
                          className={`workspace-tag workspace-tag-${tag.source || 'manual'}`}
                        >
                          {tag.name}
                          {editingAllowed ? (
                            <button type="button" onClick={() => handleRemoveTag(tag.id)} aria-label={`Remove tag ${tag.name}`}>
                              ×
                            </button>
                          ) : null}
                        </span>
                      ))}
                    </div>
                    {editingAllowed ? (
                      <form className="workspace-inline-form" onSubmit={handleAddTag}>
                        <input
                          className="form-control"
                          placeholder="Add manual tag"
                          value={tagDraft}
                          onChange={(event) => setTagDraft(event.target.value)}
                        />
                        <button type="submit" className="btn btn-outline-secondary btn-sm">
                          Add tag
                        </button>
                      </form>
                    ) : null}
                  </section>

                  <section className="workspace-bibtex-panel">
                    <div className="workspace-subsection-header">
                      <div>
                        <h3>Formatted BibTeX</h3>
                        <p className="workspace-copy">
                          Generated with the existing formatter and stored per document.
                        </p>
                      </div>
                    </div>
                    <pre>{documentDetail.bibtex_formatted || 'No BibTeX entry available yet.'}</pre>
                  </section>

                  <section className="workspace-notes-panel">
                    <div className="workspace-subsection-header">
                      <div>
                        <h3>Notes</h3>
                        <p className="workspace-copy">Collaborative notes attached to this PDF.</p>
                      </div>
                    </div>

                    {editingAllowed ? (
                      <form className="workspace-note-form" onSubmit={handleCreateNote}>
                        <textarea
                          className="form-control"
                          placeholder="Add a note for this document"
                          value={noteDraft}
                          onChange={(event) => setNoteDraft(event.target.value)}
                        />
                        <button type="submit" className="btn btn-primary btn-sm" disabled={busyAction === 'create-note'}>
                          Add note
                        </button>
                      </form>
                    ) : null}

                    <div className="workspace-note-list">
                      {(documentDetail.notes || []).length ? (
                        documentDetail.notes.map((note) => (
                          <article className="workspace-note-card" key={note.id}>
                            <header className="workspace-note-header">
                              <strong>{note.author_display_name || note.author_email}</strong>
                              {editingAllowed ? (
                                <div className="workspace-inline-actions">
                                  <button
                                    type="button"
                                    className="btn btn-outline-secondary btn-sm"
                                    onClick={() =>
                                      setNoteEdits((current) => ({
                                        ...current,
                                        [note.id]: current[note.id] ?? note.content,
                                      }))
                                    }
                                  >
                                    Edit
                                  </button>
                                  <button
                                    type="button"
                                    className="btn btn-outline-danger btn-sm"
                                    onClick={() => handleDeleteNote(note.id)}
                                  >
                                    Delete
                                  </button>
                                </div>
                              ) : null}
                            </header>

                            {noteEdits[note.id] !== undefined ? (
                              <div className="workspace-note-edit">
                                <textarea
                                  className="form-control"
                                  value={noteEdits[note.id]}
                                  onChange={(event) =>
                                    setNoteEdits((current) => ({
                                      ...current,
                                      [note.id]: event.target.value,
                                    }))
                                  }
                                />
                                <div className="workspace-inline-actions">
                                  <button
                                    type="button"
                                    className="btn btn-primary btn-sm"
                                    onClick={() => handleSaveNote(note.id)}
                                  >
                                    Save
                                  </button>
                                  <button
                                    type="button"
                                    className="btn btn-outline-secondary btn-sm"
                                    onClick={() =>
                                      setNoteEdits((current) => {
                                        const next = { ...current }
                                        delete next[note.id]
                                        return next
                                      })
                                    }
                                  >
                                    Cancel
                                  </button>
                                </div>
                              </div>
                            ) : (
                              <p>{note.content}</p>
                            )}
                          </article>
                        ))
                      ) : (
                        <div className="workspace-empty-state">No notes for this document yet.</div>
                      )}
                    </div>
                  </section>
                </>
              ) : (
                <div className="workspace-empty-state">
                  Select a document to inspect metadata, tags, notes, and the generated BibTeX entry.
                </div>
              )}
            </section>
          </section>
          </section>
        </section>
      )}

      {shareModalOpen ? (
        <div className="workspace-modal-backdrop" onClick={() => setShareModalOpen(false)}>
          <section
            className="workspace-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="workspace-share-title"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="workspace-panel-header">
              <div>
                <p className="workspace-eyebrow">Sharing</p>
                <h2 id="workspace-share-title">Project members</h2>
                <p className="workspace-copy">Invite signed-in users and manage viewer/editor access.</p>
              </div>
              <button type="button" className="workspace-modal-close" onClick={() => setShareModalOpen(false)}>
                ×
              </button>
            </div>

            <div className="workspace-share-controls">
              <input
                className="form-control"
                placeholder="Search existing users"
                value={shareSearch}
                onChange={(event) => setShareSearch(event.target.value)}
              />
              <select className="form-select" value={shareRole} onChange={(event) => setShareRole(event.target.value)}>
                <option value="viewer">Viewer</option>
                <option value="editor">Editor</option>
              </select>
            </div>

            {shareSearch.trim() ? (
              <div className="workspace-share-results">
                {shareResults.length ? (
                  shareResults.map((user) => (
                    <div className="workspace-share-result" key={user.id}>
                      <div>
                        <strong>{formatUserLabel(user)}</strong>
                        <p>{user.email}</p>
                      </div>
                      <button type="button" className="btn btn-primary btn-sm" onClick={() => handleAddMember(user.id)}>
                        Add
                      </button>
                    </div>
                  ))
                ) : (
                  <div className="workspace-empty-state">No matching signed-in users found.</div>
                )}
              </div>
            ) : null}

            <div className="workspace-member-list">
              {shareLoading ? (
                <div className="workspace-empty-state">Loading members…</div>
              ) : (
                projectMembers.map((member) => (
                  <div className="workspace-member-row" key={member.user_id}>
                    <div>
                      <strong>{member.display_name || member.email}</strong>
                      <span>{member.email}</span>
                    </div>
                    <div className="workspace-member-actions">
                      {member.role === 'owner' ? (
                        <span className="workspace-role-chip">Owner</span>
                      ) : ownerAllowed ? (
                        <>
                          <select
                            className="form-select"
                            value={member.role}
                            onChange={(event) => handleUpdateMemberRole(member.user_id, event.target.value)}
                          >
                            <option value="viewer">Viewer</option>
                            <option value="editor">Editor</option>
                          </select>
                          <button
                            type="button"
                            className="btn btn-outline-danger btn-sm"
                            onClick={() => handleRemoveMember(member.user_id)}
                          >
                            Remove
                          </button>
                        </>
                      ) : (
                        <span className="workspace-role-chip">{roleLabel(member.role)}</span>
                      )}
                    </div>
                  </div>
                ))
              )}
            </div>
          </section>
        </div>
      ) : null}
      {workspaceFooter}
    </main>
  )
}

export default WorkspaceApp
