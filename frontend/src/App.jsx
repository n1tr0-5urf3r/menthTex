import { useEffect, useRef, useState } from 'react'
import './App.css'
import LoginProviderPopup from './LoginProviderPopup'
import WorkspaceApp from './WorkspaceApp'
import { currentRelativePath, withBasePath } from './pathUtils'

const FRONTEND_VERSION = import.meta.env.VITE_APP_VERSION || '2.1.1'
const SAMPLE_INPUT = `@ARTICLE{10947349,
  author={Ihle, Fabian and Menth, Michael},
  journal={IEEE Open Journal of the Communications Society},
  title={MPLS Network Actions: Technological Overview and P4-Based Implementation on a High-Speed Switching ASIC},
  year={2025},
  volume={6},
  number={},
  pages={3480-3501},
  keywords={Multiprotocol label switching;Switches;Hardware;Encoding;Protocols;Stacking;Network slicing;Standardization;Security;Scalability;Alternate-marking method;data plane programming;IETF;MPLS network actions;multiprotocol label switching;network slicing;P4;wired communications},
  doi={10.1109/OJCOMS.2025.3557082}}`

const FORMAT_CONTRACT = [
  'By default, keys use the first two letters of the first two author surnames plus the last two year digits, with _2, _3, ... on collisions.',
  'Single-author keys use the first two letters of that surname plus the last two year digits.',
  'Titles are emitted as double-braced values and title-cased, while common prepositions and articles stay lowercase unless they are first or last.',
  'Conference booktitles and journal names are normalized to @STRING tokens from conferences.bib when possible.',
  'Missing conference or journal abbreviations are proposed for review and can then be added manually to conferences.bib.',
  'Conference pages are dropped when they are a trivial 1--N range for the whole paper; other page ranges use a double dash.',
  'Journal entries keep year, month, volume, number, numpages, and non-trivial pages when available.',
  'RFC entries are emitted as @techreport with series, number, institution, type, author, title, pagetotal, year, and month.',
  'All other fields such as abstract, keywords, doi, url, publisher, and howpublished are removed from supported entries unless Keep DOI is enabled.',
  'DOI lookup accepts plain DOIs and DOI URLs, retrieves BibTeX metadata, and formats the result with the same rules as pasted input.',
  'RFC lookup accepts values like RFC1034 or 1034, retrieves Datatracker BibTeX, repairs legacy gaps from RFC Editor metadata when needed, and formats the result as an RFC @techreport.',
  '@string and @comment entries pass through unchanged and are excluded from the diff view.',
]

const DRAFT_STORAGE_KEY = 'menthtex-draft'
const WRAPPING_EQUIVALENT_FIELDS = new Set([
  'month',
  'number',
  'numpages',
  'pages',
  'pagetotal',
  'volume',
  'year',
])

async function readJson(response) {
  const text = await response.text()
  if (!text) {
    if (response.ok) {
      return {}
    }
    throw new Error(`The server returned an empty response (HTTP ${response.status}).`)
  }

  try {
    return JSON.parse(text)
  } catch {
    throw new Error(
      `The server returned an unexpected response (HTTP ${response.status}). Please report the input that triggered this.`,
    )
  }
}

function loadDraftState() {
  try {
    const raw = window.localStorage.getItem(DRAFT_STORAGE_KEY)
    if (!raw) {
      return null
    }

    const parsed = JSON.parse(raw)
    return {
      input: typeof parsed.input === 'string' ? parsed.input : SAMPLE_INPUT,
      includeStringDefinitions:
        typeof parsed.includeStringDefinitions === 'boolean'
          ? parsed.includeStringDefinitions
          : true,
      useConferencesBib:
        typeof parsed.useConferencesBib === 'boolean' ? parsed.useConferencesBib : true,
      rewriteReferenceKey:
        typeof parsed.rewriteReferenceKey === 'boolean' ? parsed.rewriteReferenceKey : true,
      keepDoi: typeof parsed.keepDoi === 'boolean' ? parsed.keepDoi : false,
      inputMode:
        parsed.inputMode === 'doi' || parsed.inputMode === 'rfc' ? parsed.inputMode : 'paste',
      doiInput: typeof parsed.doiInput === 'string' ? parsed.doiInput : '',
      rfcInput: typeof parsed.rfcInput === 'string' ? parsed.rfcInput : '',
      loadedFilename: typeof parsed.loadedFilename === 'string' ? parsed.loadedFilename : '',
      output: typeof parsed.output === 'string' ? parsed.output : '',
      entries: Array.isArray(parsed.entries) ? parsed.entries : [],
      baseEntries: Array.isArray(parsed.baseEntries) ? parsed.baseEntries : [],
      warnings: Array.isArray(parsed.warnings) ? parsed.warnings : [],
      addedAbbreviations: Array.isArray(parsed.addedAbbreviations) ? parsed.addedAbbreviations : [],
      proposedDefinitions: Array.isArray(parsed.proposedDefinitions) ? parsed.proposedDefinitions : [],
      outputDefinitions: Array.isArray(parsed.outputDefinitions) ? parsed.outputDefinitions : [],
      baseOutputDefinitions: Array.isArray(parsed.baseOutputDefinitions) ? parsed.baseOutputDefinitions : [],
      entryCount: typeof parsed.entryCount === 'number' ? parsed.entryCount : 0,
      formattedCount: typeof parsed.formattedCount === 'number' ? parsed.formattedCount : 0,
      untouchedCount: typeof parsed.untouchedCount === 'number' ? parsed.untouchedCount : 0,
      lookupDoiLabel: typeof parsed.lookupDoiLabel === 'string' ? parsed.lookupDoiLabel : '',
      lookupRfcLabel: typeof parsed.lookupRfcLabel === 'string' ? parsed.lookupRfcLabel : '',
      pdfResolutionLabel:
        typeof parsed.pdfResolutionLabel === 'string' ? parsed.pdfResolutionLabel : '',
      showDiff: typeof parsed.showDiff === 'boolean' ? parsed.showDiff : true,
    }
  } catch {
    return null
  }
}

function persistDraftState(draft) {
  try {
    window.localStorage.setItem(DRAFT_STORAGE_KEY, JSON.stringify(draft))
  } catch {
    // Ignore localStorage write failures.
  }
}

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

function CopyIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M8 8.5A2.5 2.5 0 0 1 10.5 6h7A2.5 2.5 0 0 1 20 8.5v9a2.5 2.5 0 0 1-2.5 2.5h-7A2.5 2.5 0 0 1 8 17.5v-9Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
      />
      <path
        d="M5.5 15.5A2.5 2.5 0 0 1 3 13V6.5A2.5 2.5 0 0 1 5.5 4h6.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
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

function RejectIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M7 7 17 17M17 7 7 17"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.9"
        strokeLinecap="round"
      />
      <circle
        cx="12"
        cy="12"
        r="8.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
      />
    </svg>
  )
}

function BrowseIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M5 7.5h14M5 12h14M5 16.5h9"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <circle
        cx="18"
        cy="16.5"
        r="2.75"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.6"
      />
    </svg>
  )
}

function AddIcon() {
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

function CheckIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <circle
        cx="12"
        cy="12"
        r="8.75"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
      />
      <path
        d="m8.2 12.3 2.4 2.5 5.2-5.4"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.9"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

function buildLineDiff(originalText, outputText) {
  const originalLines = originalText.split('\n')
  const outputLines = outputText.split('\n')
  const originalHeaderLine = originalLines[0] || ''
  const outputHeaderLine = outputLines[0] || ''
  const hasHeaderPair = isEntryHeaderLine(originalHeaderLine) && isEntryHeaderLine(outputHeaderLine)
  const bodyOriginalLines = hasHeaderPair ? originalLines.slice(1) : originalLines
  const bodyOutputLines = hasHeaderPair ? outputLines.slice(1) : outputLines
  const originalBuckets = buildNormalizedLineBuckets(bodyOriginalLines)
  const outputBuckets = buildNormalizedLineBuckets(bodyOutputLines)

  const rows = []
  const suppressedNormalizations = new Set()

  if (hasHeaderPair) {
    if (originalHeaderLine !== outputHeaderLine) {
      if (normalizeHeaderLine(originalHeaderLine) === normalizeHeaderLine(outputHeaderLine)) {
        suppressedNormalizations.add('indentation')
      } else if (areEquivalentEntryHeaderLines(originalHeaderLine, outputHeaderLine)) {
        suppressedNormalizations.add('entry type')
      } else {
        rows.push({
          type: 'removed',
          left: originalHeaderLine,
          right: '',
        })
        rows.push({
          type: 'added',
          left: '',
          right: outputHeaderLine,
        })
      }
    }
  }

  for (const line of bodyOriginalLines) {
    const normalizedLine = normalizeFieldLineDiffValue(line)
    const outputBucket = outputBuckets.get(normalizedLine)
    if (outputBucket && outputBucket.count > 0) {
      const matchedLine = takeBucketMatch(outputBucket, line)
      if (matchedLine !== line) {
        classifySuppressedLineChange(line, matchedLine).forEach((changeType) =>
          suppressedNormalizations.add(changeType),
        )
      }
      continue
    }

    rows.push({
      type: 'removed',
      left: line,
      right: '',
    })
  }

  for (const line of bodyOutputLines) {
    const normalizedLine = normalizeFieldLineDiffValue(line)
    const originalBucket = originalBuckets.get(normalizedLine)
    if (originalBucket && originalBucket.count > 0) {
      takeBucketMatch(originalBucket, line)
      continue
    }

    rows.push({
      type: 'added',
      left: '',
      right: line,
    })
  }

  return {
    rows,
    suppressedNormalizations: Array.from(suppressedNormalizations),
  }
}

function buildNormalizedLineBuckets(lines) {
  const buckets = new Map()

  for (const line of lines) {
    const normalizedLine = normalizeFieldLineDiffValue(line)
    const bucket = buckets.get(normalizedLine) || {
      count: 0,
      remainingLines: [],
    }

    bucket.count += 1
    bucket.remainingLines.push(line)
    buckets.set(normalizedLine, bucket)
  }

  return buckets
}

function takeBucketMatch(bucket, preferredLine) {
  bucket.count -= 1
  const exactIndex = bucket.remainingLines.indexOf(preferredLine)
  if (exactIndex >= 0) {
    return bucket.remainingLines.splice(exactIndex, 1)[0]
  }

  return bucket.remainingLines.shift() || ''
}

function normalizeFieldLineFormattingValue(value) {
  const collapsed = (value || '').replace(/[ \t]+/g, '')
  return collapsed.includes('=') ? collapsed.replace(/,$/, '') : collapsed
}

function normalizeFieldLineDiffValue(value) {
  const collapsed = normalizeFieldLineFormattingValue(value)
  const field = parseBibtexFieldLine(collapsed)
  if (!field) {
    return collapsed
  }

  if (WRAPPING_EQUIVALENT_FIELDS.has(field.name)) {
    return `${field.name}=${stripOuterWrapping(field.rawValue)}`
  }

  return `${field.name}=${field.rawValue}`
}

function classifySuppressedLineChange(left, right) {
  if (left === right) {
    return []
  }

  if (normalizeFieldLineFormattingValue(left) === normalizeFieldLineFormattingValue(right)) {
    return ['indentation']
  }

  const leftField = parseBibtexFieldLine(normalizeFieldLineFormattingValue(left))
  const rightField = parseBibtexFieldLine(normalizeFieldLineFormattingValue(right))

  if (
    leftField &&
    rightField &&
    leftField.name === rightField.name &&
    WRAPPING_EQUIVALENT_FIELDS.has(leftField.name) &&
    normalizeFieldLineDiffValue(left) === normalizeFieldLineDiffValue(right)
  ) {
    return ['wrapping']
  }

  return []
}

function parseBibtexFieldLine(value) {
  const fieldMatch = (value || '').match(/^([^=]+)=(.+)$/)
  if (!fieldMatch) {
    return null
  }

  return {
    name: fieldMatch[1].toLowerCase(),
    rawValue: fieldMatch[2],
  }
}

function isEntryHeaderLine(value) {
  return /^\s*@[\w-]+\s*[{(]/.test(value || '')
}

function normalizeHeaderLine(value) {
  return (value || '').replace(/[ \t]+/g, '')
}

function areEquivalentEntryHeaderLines(left, right) {
  const leftHeader = parseEntryHeaderLine(left)
  const rightHeader = parseEntryHeaderLine(right)
  if (!leftHeader || !rightHeader) {
    return false
  }

  return leftHeader.type === rightHeader.type && leftHeader.key === rightHeader.key
}

function parseEntryHeaderLine(value) {
  const match = (value || '').trim().match(/^@([\w-]+)\s*[{(]\s*([^,]+)\s*,\s*$/)
  if (!match) {
    return null
  }

  return {
    type: match[1].toLowerCase(),
    key: match[2].trim(),
  }
}

function summarizeWarnings(warnings) {
  const grouped = new Map()

  for (const warning of warnings) {
    const match = warning.match(/^Left entry '([^']+)' untouched because (.+)\.$/)
    if (!match) {
      const current = grouped.get(warning) || { reason: warning, entries: [] }
      grouped.set(warning, current)
      continue
    }

    const [, entryKey, reason] = match
    const current = grouped.get(reason) || { reason, entries: [] }
    current.entries.push(entryKey)
    grouped.set(reason, current)
  }

  return Array.from(grouped.values()).map((group) => {
    if (group.entries.length === 0) {
      return group.reason
    }

    const sample = group.entries.slice(0, 3).join(', ')
    const remainder = group.entries.length > 3 ? ` +${group.entries.length - 3} more` : ''
    return `${group.entries.length} entries untouched: ${group.reason} (${sample}${remainder})`
  })
}

function buildWarningHeadline(warnings, warningSummaries) {
  if (warnings.length === 0) {
    return ''
  }
  if (warningSummaries.length === 1) {
    return warningSummaries[0]
  }
  return `${warnings.length} warnings across ${warningSummaries.length} issues.`
}

function shouldShowDiffEntry(entry) {
  const hiddenTypes = new Set(['string', 'comment'])
  return !hiddenTypes.has((entry.entry_type || '').toLowerCase())
}

function buildDefinitionText(token, rawValue) {
  return `@STRING{${token.trim()} = ${rawValue.trim()}}`
}

function stripTrailingCanonicalTokenSuffix(value) {
  const normalizedValue = stripOuterWrapping(value)
  const match = normalizedValue.match(/\s+\(([A-Za-z][A-Za-z0-9_-]*)\)$/)
  return match ? normalizedValue.slice(0, -match[0].length).trimEnd() : normalizedValue
}

function buildCanonicalDefinitionRawValue(expandedValue, token) {
  const normalizedToken = token.trim()
  const baseValue = stripTrailingCanonicalTokenSuffix(expandedValue)
  return `{{${baseValue} (${normalizedToken})}}`
}

function stripSpecificTokenSuffix(expandedValue, token) {
  const normalizedValue = stripOuterWrapping(expandedValue)
  const normalizedToken = token.trim()
  const suffix = `(${normalizedToken})`
  return normalizedValue.endsWith(suffix)
    ? normalizedValue.slice(0, -suffix.length).trimEnd()
    : normalizedValue
}

function stripOuterWrapping(raw) {
  let current = (raw || '').trim()
  while (current.length >= 2) {
    const wrappedInBraces = current.startsWith('{') && current.endsWith('}')
    const wrappedInQuotes = current.startsWith('"') && current.endsWith('"')
    if (!(wrappedInBraces || wrappedInQuotes)) {
      break
    }
    current = current.slice(1, -1).trim()
  }
  return current
}

function extractExpandedValue(rawValue) {
  return stripOuterWrapping(rawValue)
}

function hasBalancedBraces(value) {
  let depth = 0
  for (const char of value) {
    if (char === '{') depth += 1
    if (char === '}') {
      depth -= 1
      if (depth < 0) return false
    }
  }
  return depth === 0
}

function validateDefinitionDraft(token, rawValue) {
  const normalizedToken = token.trim()
  const normalizedValue = rawValue.trim()

  if (!normalizedToken) {
    return 'Token must not be empty.'
  }
  if (!/^[A-Za-z][A-Za-z0-9_-]*$/.test(normalizedToken)) {
    return 'Token must start with a letter and use only letters, numbers, "_" or "-".'
  }
  if (!normalizedValue) {
    return 'Definition value must not be empty.'
  }
  if (
    !(normalizedValue.startsWith('{') && normalizedValue.endsWith('}')) &&
    !(normalizedValue.startsWith('"') && normalizedValue.endsWith('"'))
  ) {
    return 'Definition value should be wrapped in braces or quotes.'
  }
  if (!hasBalancedBraces(normalizedValue)) {
    return 'Definition value contains unbalanced braces.'
  }

  return ''
}

function createProposalDraft(definition, index) {
  return {
    ...definition,
    id: `${definition.token}-${index}`,
    original_token: definition.token,
    token: definition.token,
    raw_value: definition.raw_value,
    expanded_value: definition.expanded_value,
    definition_text: definition.definition_text,
    suggestions: definition.suggestions || [],
    isEditing: false,
    validationError: '',
    resolution: 'pending',
    resolved_definition: null,
    isCollapsed: false,
  }
}

function normalizeProposedDefinitions(definitions) {
  return (definitions || []).map((definition, index) => createProposalDraft(definition, index))
}

function createManagedDefinitionDraft(definition, index) {
  const longName = stripSpecificTokenSuffix(definition.expanded_value, definition.token)
  const rawValue = buildCanonicalDefinitionRawValue(longName, definition.token)
  return {
    id: `managed-${definition.token}-${index}`,
    originalToken: definition.token,
    token: definition.token,
    longName,
    raw_value: rawValue,
    expanded_value: definition.expanded_value,
    definition_text: buildDefinitionText(definition.token, rawValue),
    isEditing: false,
    validationError: validateDefinitionDraft(definition.token, rawValue),
  }
}

function createNewManagedDefinitionDraft() {
  return {
    id: 'new-definition',
    originalToken: '',
    token: '',
    longName: '',
    raw_value: '',
    expanded_value: '',
    definition_text: '',
    isEditing: true,
    validationError: 'Token must not be empty.',
  }
}

function createOutputDefinitionDraft(definition, index) {
  return {
    ...definition,
    id: `output-${definition.token}-${index}`,
    token: definition.token,
    raw_value: definition.raw_value,
    expanded_value: definition.expanded_value,
    definition_text: definition.definition_text,
    isEditing: false,
    validationError: '',
  }
}

function createOutputDefinitionFromSuggestion(definition, index) {
  return createOutputDefinitionDraft(
    {
      token: definition.token,
      raw_value: definition.raw_value,
      expanded_value: definition.expanded_value,
      definition_text: definition.definition_text,
    },
    index,
  )
}

function definitionResolutionMeta(definition) {
  if (definition.resolution === 'rejected') {
    return {
      label: 'Resolved: literal venue',
      className: 'definition-resolution-rejected',
    }
  }
  if (definition.resolution === 'existing' && definition.resolved_definition) {
    return {
      label: `Resolved: ${definition.resolved_definition.token}`,
      className: 'definition-resolution-existing',
    }
  }
  if (definition.resolution === 'added') {
    return {
      label: 'Resolved: added',
      className: 'definition-resolution-added',
    }
  }
  return {
    label: 'Pending review',
    className: 'definition-resolution-pending',
  }
}

function isDefinitionResolved(definition) {
  return definition.resolution && definition.resolution !== 'pending'
}

function buildOutputDefinitionState(outputDefinitions, proposedDefinitions) {
  const definitionsByToken = new Map(
    proposedDefinitions.map((definition) => [definition.token.toUpperCase(), definition]),
  )

  const mergedDefinitions = (outputDefinitions || []).map((definition, index) => {
    const matchingProposal = definitionsByToken.get(definition.token.toUpperCase())
    return matchingProposal || createOutputDefinitionDraft(definition, index)
  })

  for (const proposal of proposedDefinitions) {
    const alreadyIncluded = mergedDefinitions.some(
      (definition) => definition.token.toUpperCase() === proposal.token.toUpperCase(),
    )
    if (!alreadyIncluded) {
      mergedDefinitions.push(proposal)
    }
  }

  return mergedDefinitions
}

function replacePublicationTokenInEntryOutput(outputText, previousToken, nextToken) {
  return outputText
    .replaceAll(`journal = ${previousToken}`, `journal = ${nextToken}`)
    .replaceAll(`booktitle = ${previousToken}`, `booktitle = ${nextToken}`)
}

function replacePublicationTokenWithLiteralValue(outputText, token, expandedValue) {
  const literalValue = `{${stripSpecificTokenSuffix(expandedValue, token)}}`
  return outputText
    .replaceAll(`journal = ${token}`, `journal = ${literalValue}`)
    .replaceAll(`booktitle = ${token}`, `booktitle = ${literalValue}`)
}

function rebuildOutputDocument(entries, proposedDefinitions, includeStringDefinitions) {
  const normalizeDocumentBlock = (value) =>
    (value || '')
      .replace(/\r\n/g, '\n')
      .split('\n')
      .map((line) => line.replace(/[ \t]+$/g, ''))
      .join('\n')
      .trim()

  const sections = []
  if (includeStringDefinitions) {
    const definitionTexts = proposedDefinitions
      .map((definition) => normalizeDocumentBlock(definition.definition_text))
      .filter(Boolean)
    if (definitionTexts.length > 0) {
      sections.push(definitionTexts.join('\n'))
    }
  }

  const entryTexts = entries
    .map((entry) => normalizeDocumentBlock(entry.output_text))
    .filter(Boolean)
  if (entryTexts.length > 0) {
    sections.push(entryTexts.join('\n\n'))
  }

  return sections.join('\n\n').replace(/\n{3,}/g, '\n\n')
}

function deriveResolvedOutputState(baseEntries, baseOutputDefinitions, proposedDefinitions) {
  const nextEntries = (baseEntries || []).map((entry) => ({
    ...entry,
    output_text: entry.base_output_text || entry.output_text,
  }))
  const nextOutputDefinitions = [...(baseOutputDefinitions || [])]

  for (const definition of proposedDefinitions || []) {
    const originalToken = definition.original_token || definition.token
    const currentToken = definition.token

    for (const entry of nextEntries) {
      if (entry.status !== 'formatted') {
        continue
      }

      let nextOutputText = entry.output_text
      if (originalToken !== currentToken) {
        nextOutputText = replacePublicationTokenInEntryOutput(
          nextOutputText,
          originalToken,
          currentToken,
        )
      }

      if (definition.resolution === 'rejected') {
        nextOutputText = replacePublicationTokenWithLiteralValue(
          nextOutputText,
          currentToken,
          definition.expanded_value,
        )
      } else if (definition.resolution === 'existing' && definition.resolved_definition) {
        nextOutputText = replacePublicationTokenInEntryOutput(
          nextOutputText,
          currentToken,
          definition.resolved_definition.token,
        )
      }

      entry.output_text = nextOutputText
    }

    const currentTokenKey = currentToken.toUpperCase()
    const originalTokenKey = originalToken.toUpperCase()
    const existingIndex = nextOutputDefinitions.findIndex(
      (candidate) =>
        candidate.token.toUpperCase() === currentTokenKey
        || candidate.token.toUpperCase() === originalTokenKey,
    )

    if (definition.resolution === 'rejected') {
      if (existingIndex >= 0) {
        nextOutputDefinitions.splice(existingIndex, 1)
      }
      continue
    }

    if (definition.resolution === 'existing' && definition.resolved_definition) {
      if (existingIndex >= 0) {
        nextOutputDefinitions.splice(existingIndex, 1)
      }
      const resolvedTokenKey = definition.resolved_definition.token.toUpperCase()
      const hasResolvedDefinition = nextOutputDefinitions.some(
        (candidate) => candidate.token.toUpperCase() === resolvedTokenKey,
      )
      if (!hasResolvedDefinition) {
        nextOutputDefinitions.push(
          createOutputDefinitionFromSuggestion(
            definition.resolved_definition,
            nextOutputDefinitions.length,
          ),
        )
      }
      continue
    }

    const proposalDefinition = {
      ...definition,
      id: `output-${currentToken}-${nextOutputDefinitions.length}`,
    }
    if (existingIndex >= 0) {
      nextOutputDefinitions[existingIndex] = proposalDefinition
    } else {
      nextOutputDefinitions.push(proposalDefinition)
    }
  }

  return {
    entries: nextEntries,
    outputDefinitions: nextOutputDefinitions,
  }
}

function applyFormatPayload(payload, setState) {
  const {
    setOutput,
    setEntries,
    setBaseEntries,
    setWarnings,
    setAddedAbbreviations,
    setProposedDefinitions,
    setOutputDefinitions,
    setBaseOutputDefinitions,
    setEntryCount,
    setFormattedCount,
    setUntouchedCount,
    setExpandedUntouched,
    setExpandedFullEntries,
    includeStringDefinitions,
  } = setState

  const nextEntries = (payload.entries || []).map((entry) => ({
    ...entry,
    base_output_text: entry.output_text,
  }))
  const nextDefinitions = normalizeProposedDefinitions(payload.proposed_definitions || [])
  const nextBaseOutputDefinitions = buildOutputDefinitionState(
    payload.output_definitions || [],
    [],
  )
  const nextOutputDefinitions = buildOutputDefinitionState(
    nextBaseOutputDefinitions,
    nextDefinitions,
  )
  setEntries(nextEntries)
  setBaseEntries(nextEntries)
  setWarnings(payload.warnings || [])
  setAddedAbbreviations(payload.added_abbreviations || [])
  setProposedDefinitions(nextDefinitions)
  setOutputDefinitions(nextOutputDefinitions)
  setBaseOutputDefinitions(nextBaseOutputDefinitions)
  setEntryCount(payload.entry_count || 0)
  setFormattedCount(payload.formatted_count || 0)
  setUntouchedCount(payload.untouched_count || 0)
  setExpandedUntouched({})
  setExpandedFullEntries({})
  setOutput(rebuildOutputDocument(nextEntries, nextOutputDefinitions, includeStringDefinitions))
}

function FormatterApp() {
  const [draftState] = useState(loadDraftState)
  const [authStatus, setAuthStatus] = useState({
    enabled: false,
    authenticated: false,
    can_manage_abbreviations: false,
    providers: [],
    email: null,
    loaded: false,
  })
  const [authFlash, setAuthFlash] = useState(null)
  const [theme, setTheme] = useState(() => {
    const storedTheme = window.localStorage.getItem('menthtex-theme')
    if (storedTheme === 'light' || storedTheme === 'dark') {
      return storedTheme
    }

    return window.matchMedia('(prefers-color-scheme: dark)').matches
      ? 'dark'
      : 'light'
  })
  const [input, setInput] = useState(draftState?.input || SAMPLE_INPUT)
  const [includeStringDefinitions, setIncludeStringDefinitions] = useState(
    draftState?.includeStringDefinitions ?? true,
  )
  const [useConferencesBib, setUseConferencesBib] = useState(
    draftState?.useConferencesBib ?? true,
  )
  const [rewriteReferenceKey, setRewriteReferenceKey] = useState(
    draftState?.rewriteReferenceKey ?? true,
  )
  const [keepDoi, setKeepDoi] = useState(draftState?.keepDoi ?? false)
  const [inputMode, setInputMode] = useState(draftState?.inputMode || 'paste')
  const [output, setOutput] = useState(draftState?.output || '')
  const [entries, setEntries] = useState(draftState?.entries || [])
  const [baseEntries, setBaseEntries] = useState(draftState?.baseEntries || [])
  const [warnings, setWarnings] = useState(draftState?.warnings || [])
  const [addedAbbreviations, setAddedAbbreviations] = useState(
    draftState?.addedAbbreviations || [],
  )
  const [proposedDefinitions, setProposedDefinitions] = useState(
    draftState?.proposedDefinitions || [],
  )
  const [outputDefinitions, setOutputDefinitions] = useState(
    draftState?.outputDefinitions || [],
  )
  const [baseOutputDefinitions, setBaseOutputDefinitions] = useState(
    draftState?.baseOutputDefinitions || [],
  )
  const [error, setError] = useState('')
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [copyLabel, setCopyLabel] = useState('Copy output')
  const [loadedFilename, setLoadedFilename] = useState(draftState?.loadedFilename || '')
  const [doiInput, setDoiInput] = useState(draftState?.doiInput || '')
  const [rfcInput, setRfcInput] = useState(draftState?.rfcInput || '')
  const [entryCount, setEntryCount] = useState(draftState?.entryCount || 0)
  const [formattedCount, setFormattedCount] = useState(draftState?.formattedCount || 0)
  const [untouchedCount, setUntouchedCount] = useState(draftState?.untouchedCount || 0)
  const [expandedUntouched, setExpandedUntouched] = useState({})
  const [expandedFullEntries, setExpandedFullEntries] = useState({})
  const [showHelper, setShowHelper] = useState(false)
  const [isUploadingPdf, setIsUploadingPdf] = useState(false)
  const [lookupDoiLabel, setLookupDoiLabel] = useState(draftState?.lookupDoiLabel || '')
  const [lookupRfcLabel, setLookupRfcLabel] = useState(draftState?.lookupRfcLabel || '')
  const [pdfResolutionLabel, setPdfResolutionLabel] = useState(
    draftState?.pdfResolutionLabel || '',
  )
  const [showDiff, setShowDiff] = useState(draftState?.showDiff ?? true)
  const [isDragActive, setIsDragActive] = useState(false)
  const [savingDefinitionId, setSavingDefinitionId] = useState('')
  const [definitionOptions, setDefinitionOptions] = useState([])
  const [definitionOptionsLoading, setDefinitionOptionsLoading] = useState(false)
  const [definitionOptionsError, setDefinitionOptionsError] = useState('')
  const [activeDefinitionChooserId, setActiveDefinitionChooserId] = useState('')
  const [definitionOptionQuery, setDefinitionOptionQuery] = useState('')
  const [showDefinitionManager, setShowDefinitionManager] = useState(false)
  const [managedDefinitions, setManagedDefinitions] = useState([])
  const [definitionSearch, setDefinitionSearch] = useState('')
  const [definitionManagerError, setDefinitionManagerError] = useState('')
  const [definitionsLoading, setDefinitionsLoading] = useState(false)
  const [definitionMutationId, setDefinitionMutationId] = useState('')
  const [newManagedDefinition, setNewManagedDefinition] = useState(createNewManagedDefinitionDraft)
  const [versionInfo, setVersionInfo] = useState({
    appVersion: FRONTEND_VERSION,
    backendVersion: '',
  })
  const dropzoneInputRef = useRef(null)
  const warningSummaries = summarizeWarnings(warnings)
  const warningHeadline = buildWarningHeadline(warnings, warningSummaries)
  const diffEntries = entries.filter(shouldShowDiffEntry)
  const canManageAbbreviations = authStatus.authenticated && authStatus.can_manage_abbreviations
  const filteredManagedDefinitions = managedDefinitions.filter((definition) => {
    const query = definitionSearch.trim().toLowerCase()
    if (!query) {
      return true
    }
    return (
      definition.token.toLowerCase().includes(query)
      || definition.longName.toLowerCase().includes(query)
    )
  })

  useEffect(() => {
    document.documentElement.dataset.theme = theme
    window.localStorage.setItem('menthtex-theme', theme)
  }, [theme])

  useEffect(() => {
    setOutput(rebuildOutputDocument(entries, outputDefinitions, includeStringDefinitions))
  }, [entries, outputDefinitions, includeStringDefinitions])

  useEffect(() => {
    persistDraftState({
      input,
      includeStringDefinitions,
      useConferencesBib,
      rewriteReferenceKey,
      keepDoi,
      inputMode,
      doiInput,
      rfcInput,
      loadedFilename,
      output,
      entries,
      baseEntries,
      warnings,
      addedAbbreviations,
      proposedDefinitions,
      outputDefinitions,
      baseOutputDefinitions,
      entryCount,
      formattedCount,
      untouchedCount,
      lookupDoiLabel,
      lookupRfcLabel,
      pdfResolutionLabel,
      showDiff,
    })
  }, [
    input,
    includeStringDefinitions,
    useConferencesBib,
    rewriteReferenceKey,
    keepDoi,
    inputMode,
    doiInput,
    rfcInput,
    loadedFilename,
    output,
    entries,
    baseEntries,
    warnings,
    addedAbbreviations,
    proposedDefinitions,
    outputDefinitions,
    baseOutputDefinitions,
    entryCount,
    formattedCount,
    untouchedCount,
    lookupDoiLabel,
    lookupRfcLabel,
    pdfResolutionLabel,
    showDiff,
  ])

  useEffect(() => {
    async function loadVersionInfo() {
      try {
        const response = await fetch(withBasePath('api/health'))
        const payload = await readJson(response)
        if (!response.ok) {
          throw new Error(payload.error || 'Failed to load version information.')
        }
        setVersionInfo({
          appVersion: payload.app_version || FRONTEND_VERSION,
          backendVersion: payload.backend_version || '',
        })
      } catch {
        setVersionInfo({
          appVersion: FRONTEND_VERSION,
          backendVersion: '',
        })
      }
    }

    loadVersionInfo()
  }, [])

  useEffect(() => {
    async function loadAuthStatus() {
      try {
        const response = await fetch(withBasePath('api/auth/me'))
        const payload = await readJson(response)
        if (!response.ok) {
          throw new Error(payload.error || 'Failed to load authentication status.')
        }
        setAuthStatus({
          enabled: payload.enabled,
          authenticated: payload.authenticated,
          can_manage_abbreviations: payload.can_manage_abbreviations,
          providers: Array.isArray(payload.providers) ? payload.providers : [],
          email: payload.email || null,
          loaded: true,
        })
      } catch {
        setAuthStatus({
          enabled: false,
          authenticated: false,
          can_manage_abbreviations: false,
          providers: [],
          email: null,
          loaded: true,
        })
      }
    }

    loadAuthStatus()
  }, [])

  useEffect(() => {
    const url = new URL(window.location.href)
    const auth = url.searchParams.get('auth')
    const authError = url.searchParams.get('auth_error')

    if (!auth && !authError) {
      return
    }

    if (authError) {
      setAuthFlash({ type: 'error', message: authError })
    } else if (auth === 'success') {
      setAuthFlash({ type: 'success', message: 'Signed in.' })
    }

    url.searchParams.delete('auth')
    url.searchParams.delete('auth_error')
    window.history.replaceState({}, '', `${url.pathname}${url.search}${url.hash}`)
  }, [])

  useEffect(() => {
    if (!authFlash) {
      return undefined
    }

    const timer = window.setTimeout(() => setAuthFlash(null), 7000)
    return () => window.clearTimeout(timer)
  }, [authFlash])

  async function refreshAuthStatus() {
    const response = await fetch(withBasePath('api/auth/me'))
    const payload = await readJson(response)
    if (!response.ok) {
      throw new Error(payload.error || 'Failed to load authentication status.')
    }
    setAuthStatus({
      enabled: payload.enabled,
      authenticated: payload.authenticated,
      can_manage_abbreviations: payload.can_manage_abbreviations,
      providers: Array.isArray(payload.providers) ? payload.providers : [],
      email: payload.email || null,
      loaded: true,
    })
  }

  function handleLoginProvider(providerId) {
    const currentUrl = new URL(window.location.href)
    currentUrl.searchParams.delete('auth')
    currentUrl.searchParams.delete('auth_error')
    const returnTo = `${currentUrl.pathname}${currentUrl.search}${currentUrl.hash}`
    window.location.assign(
      `${withBasePath(`api/auth/login/${providerId}`)}?return_to=${encodeURIComponent(returnTo)}`,
    )
  }

  async function handleLogout() {
    try {
      const response = await fetch(withBasePath('api/auth/logout'), {
        method: 'POST',
      })
      const payload = await readJson(response)
      if (!response.ok) {
        throw new Error(payload.error || 'Failed to sign out.')
      }
      await refreshAuthStatus()
      setAuthFlash({ type: 'info', message: 'Signed out.' })
    } catch (requestError) {
      setError(requestError.message)
    }
  }

  function updateManagedDefinitionDraft(definition, field, value) {
    const nextToken = field === 'token' ? value : definition.token
    const nextLongName = field === 'longName' ? value : definition.longName
    const nextRawValue =
      nextToken.trim() && nextLongName.trim()
        ? buildCanonicalDefinitionRawValue(nextLongName, nextToken)
        : ''
    return {
      ...definition,
      token: nextToken,
      longName: nextLongName,
      raw_value: nextRawValue,
      expanded_value: nextLongName.trim() && nextToken.trim()
        ? extractExpandedValue(nextRawValue)
        : '',
      definition_text:
        nextToken.trim() && nextRawValue
          ? buildDefinitionText(nextToken, nextRawValue)
          : '',
      validationError: validateDefinitionDraft(nextToken, nextRawValue),
    }
  }

  async function loadManagedDefinitions() {
    setDefinitionsLoading(true)
    setDefinitionManagerError('')
    try {
      const response = await fetch(withBasePath('api/definitions'))
      const payload = await readJson(response)
      if (!response.ok) {
        throw new Error(payload.error || 'Failed to load conferences.bib definitions.')
      }
      setManagedDefinitions(
        (payload.definitions || []).map((definition, index) =>
          createManagedDefinitionDraft(definition, index),
        ),
      )
      setNewManagedDefinition(createNewManagedDefinitionDraft())
    } catch (requestError) {
      setDefinitionManagerError(requestError.message)
    } finally {
      setDefinitionsLoading(false)
    }
  }

  async function openDefinitionManager() {
    if (!canManageAbbreviations) {
      setError(
        'Sign in as a kn-staff or kn-esslingen member to manage conferences.bib.',
      )
      return
    }

    setShowDefinitionManager(true)
    await loadManagedDefinitions()
  }

  function handleManagedDefinitionFieldChange(definitionId, field, value) {
    setManagedDefinitions((current) =>
      current.map((definition) =>
        definition.id === definitionId
          ? updateManagedDefinitionDraft(definition, field, value)
          : definition,
      ),
    )
  }

  function handleNewDefinitionFieldChange(field, value) {
    setNewManagedDefinition((current) => updateManagedDefinitionDraft(current, field, value))
  }

  function toggleManagedDefinitionEditing(definitionId) {
    setManagedDefinitions((current) =>
      current.map((definition) =>
        definition.id === definitionId
          ? { ...definition, isEditing: !definition.isEditing }
          : definition,
      ),
    )
  }

  async function handleCreateManagedDefinition() {
    if (newManagedDefinition.validationError) {
      setDefinitionManagerError(newManagedDefinition.validationError)
      return
    }

    setDefinitionMutationId(newManagedDefinition.id)
    setDefinitionManagerError('')
    try {
      const response = await fetch(withBasePath('api/definitions'), {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          token: newManagedDefinition.token,
          expanded_value: newManagedDefinition.longName,
        }),
      })
      const payload = await readJson(response)
      if (!response.ok) {
        throw new Error(payload.error || 'Failed to create definition.')
      }

      const created = createManagedDefinitionDraft(payload.definition, Date.now())
      setManagedDefinitions((current) =>
        [...current, created].sort((left, right) => left.token.localeCompare(right.token)),
      )
      setNewManagedDefinition(createNewManagedDefinitionDraft())
      setAuthFlash({ type: 'success', message: `Added ${created.token} to conferences.bib.` })
    } catch (requestError) {
      setDefinitionManagerError(requestError.message)
    } finally {
      setDefinitionMutationId('')
    }
  }

  async function handleSaveManagedDefinition(definition) {
    if (definition.validationError) {
      setDefinitionManagerError(definition.validationError)
      return
    }

    setDefinitionMutationId(definition.id)
    setDefinitionManagerError('')
    try {
      const response = await fetch(
        withBasePath(`api/definitions/${encodeURIComponent(definition.originalToken)}`),
        {
          method: 'PUT',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({
            token: definition.token,
            expanded_value: definition.longName,
          }),
        },
      )
      const payload = await readJson(response)
      if (!response.ok) {
        throw new Error(payload.error || 'Failed to update definition.')
      }

      const updated = createManagedDefinitionDraft(payload.definition, Date.now())
      setManagedDefinitions((current) =>
        current
          .map((candidate) => (candidate.id === definition.id ? updated : candidate))
          .sort((left, right) => left.token.localeCompare(right.token)),
      )
      setAuthFlash({ type: 'success', message: `Updated ${updated.token} in conferences.bib.` })
    } catch (requestError) {
      setDefinitionManagerError(requestError.message)
    } finally {
      setDefinitionMutationId('')
    }
  }

  async function handleDeleteManagedDefinition(definition) {
    const confirmed = window.confirm(`Delete ${definition.token} from conferences.bib?`)
    if (!confirmed) {
      return
    }

    setDefinitionMutationId(definition.id)
    setDefinitionManagerError('')
    try {
      const response = await fetch(
        withBasePath(`api/definitions/${encodeURIComponent(definition.originalToken)}`),
        {
          method: 'DELETE',
        },
      )
      if (!response.ok) {
        const payload = await readJson(response)
        throw new Error(payload.error || 'Failed to delete definition.')
      }

      setManagedDefinitions((current) =>
        current.filter((candidate) => candidate.id !== definition.id),
      )
      setAuthFlash({ type: 'info', message: `Deleted ${definition.token} from conferences.bib.` })
    } catch (requestError) {
      setDefinitionManagerError(requestError.message)
    } finally {
      setDefinitionMutationId('')
    }
  }

  async function handleSubmit(event) {
    event.preventDefault()

    if (inputMode === 'doi') {
      setIsSubmitting(true)
      try {
        await handleDoiLookup()
      } finally {
        setIsSubmitting(false)
      }
      return
    }

    if (inputMode === 'rfc') {
      setIsSubmitting(true)
      try {
        await handleRfcLookup()
      } finally {
        setIsSubmitting(false)
      }
      return
    }

    setIsSubmitting(true)
    setError('')
    setWarnings([])
    setAddedAbbreviations([])
    setProposedDefinitions([])
    setOutputDefinitions([])
    setPdfResolutionLabel('')

    try {
      const response = await fetch(withBasePath('api/format-entry'), {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          input,
          include_string_definitions: includeStringDefinitions,
          use_conferences_bib: useConferencesBib,
          rewrite_reference_key: rewriteReferenceKey,
          keep_doi: keepDoi,
        }),
      })

      const payload = await readJson(response)
      if (!response.ok) {
        throw new Error(payload.error || 'Formatting failed.')
      }

      applyFormatPayload(payload, {
        setOutput,
        setEntries,
        setBaseEntries,
        setWarnings,
        setAddedAbbreviations,
        setProposedDefinitions,
        setOutputDefinitions,
        setBaseOutputDefinitions,
      setEntryCount,
      setFormattedCount,
      setUntouchedCount,
      setExpandedUntouched,
      setExpandedFullEntries,
      includeStringDefinitions,
    })
      setLookupDoiLabel('')
      setLookupRfcLabel('')
      setPdfResolutionLabel('')
      setShowDiff(true)
    } catch (requestError) {
      setOutput('')
      setEntries([])
      setEntryCount(0)
      setFormattedCount(0)
      setUntouchedCount(0)
      setProposedDefinitions([])
      setOutputDefinitions([])
      setError(requestError.message)
    } finally {
      setIsSubmitting(false)
    }
  }

  async function handleDoiLookup() {
    if (!doiInput.trim()) {
      setError('Please enter a DOI or DOI URL.')
      return
    }

    setError('')
    setWarnings([])
    setAddedAbbreviations([])
    setProposedDefinitions([])
    setOutputDefinitions([])
    setLookupRfcLabel('')
    setPdfResolutionLabel('')

    try {
      const response = await fetch(withBasePath('api/lookup-doi'), {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          doi: doiInput,
          include_string_definitions: includeStringDefinitions,
          use_conferences_bib: useConferencesBib,
          rewrite_reference_key: rewriteReferenceKey,
          keep_doi: keepDoi,
        }),
      })

      const payload = await readJson(response)
      if (!response.ok) {
        throw new Error(payload.error || 'DOI lookup failed.')
      }

      applyFormatPayload(payload, {
        setOutput,
        setEntries,
        setBaseEntries,
        setWarnings,
        setAddedAbbreviations,
        setProposedDefinitions,
        setOutputDefinitions,
        setBaseOutputDefinitions,
      setEntryCount,
      setFormattedCount,
      setUntouchedCount,
      setExpandedUntouched,
      setExpandedFullEntries,
      includeStringDefinitions,
    })
      if (payload.retrieved_input) {
        setInput(payload.retrieved_input)
      }
      setLoadedFilename('')
      setLookupDoiLabel(payload.normalized_doi || doiInput.trim())
      setLookupRfcLabel('')
      setPdfResolutionLabel('')
      setInputMode('doi')
      setShowDiff(true)
    } catch (requestError) {
      setError(requestError.message)
    }
  }

  async function handleRfcLookup() {
    if (!rfcInput.trim()) {
      setError('Please enter an RFC number like RFC1034 or 1034.')
      return
    }

    setError('')
    setWarnings([])
    setAddedAbbreviations([])
    setProposedDefinitions([])
    setOutputDefinitions([])
    setLookupDoiLabel('')
    setPdfResolutionLabel('')

    try {
      const response = await fetch(withBasePath('api/lookup-rfc'), {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          rfc: rfcInput,
          include_string_definitions: includeStringDefinitions,
          use_conferences_bib: useConferencesBib,
          rewrite_reference_key: rewriteReferenceKey,
          keep_doi: keepDoi,
        }),
      })

      const payload = await readJson(response)
      if (!response.ok) {
        throw new Error(payload.error || 'RFC lookup failed.')
      }

      applyFormatPayload(payload, {
        setOutput,
        setEntries,
        setBaseEntries,
        setWarnings,
        setAddedAbbreviations,
        setProposedDefinitions,
        setOutputDefinitions,
        setBaseOutputDefinitions,
        setEntryCount,
        setFormattedCount,
        setUntouchedCount,
        setExpandedUntouched,
        setExpandedFullEntries,
        includeStringDefinitions,
      })
      if (payload.retrieved_input) {
        setInput(payload.retrieved_input)
      }
      setLoadedFilename('')
      setLookupRfcLabel(payload.normalized_rfc || rfcInput.trim())
      setInputMode('rfc')
      setShowDiff(true)
    } catch (requestError) {
      setError(requestError.message)
    }
  }

  async function handleCopy() {
    if (!output) {
      return
    }

    await navigator.clipboard.writeText(output)
    setCopyLabel('Copied')
    window.setTimeout(() => setCopyLabel('Copy output'), 1500)
  }

  function buildDownloadFilename() {
    if (loadedFilename) {
      return loadedFilename
    }

    if (entries.length === 1 && entries[0]?.key) {
      return `${entries[0].key}.bib`
    }

    return 'formatted-output.bib'
  }

  function handleDownload() {
    if (!output) {
      return
    }

    const blob = new Blob([output], { type: 'text/plain;charset=utf-8' })
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = buildDownloadFilename()
    link.click()
    URL.revokeObjectURL(url)
  }

  async function handleFileChange(event) {
    const [file] = event.target.files || []
    if (!file) {
      return
    }

    const text = await file.text()
    setInput(text)
    setLoadedFilename(file.name)
    setLookupDoiLabel('')
    setLookupRfcLabel('')
    setPdfResolutionLabel('')
    setInputMode('paste')
  }

  async function handleDropzoneFileChange(event) {
    const [file] = event.target.files || []
    if (!file) {
      return
    }

    if (inputMode === 'paste') {
      await handleFileChange(event)
    } else if (inputMode === 'doi') {
      await handlePdfFileChange(event)
    }
  }

  function openDropzonePicker() {
    if (inputMode === 'rfc') {
      return
    }
    dropzoneInputRef.current?.click()
  }

  function handleClearInput() {
    setInput('')
    setDoiInput('')
    setRfcInput('')
    setLoadedFilename('')
    setLookupDoiLabel('')
    setLookupRfcLabel('')
    setPdfResolutionLabel('')
    setError('')
  }

  function handleClearOutput() {
    setOutput('')
    setEntries([])
    setBaseEntries([])
    setWarnings([])
    setAddedAbbreviations([])
    setProposedDefinitions([])
    setOutputDefinitions([])
    setBaseOutputDefinitions([])
    setEntryCount(0)
    setFormattedCount(0)
    setUntouchedCount(0)
    setSavingDefinitionId('')
    setActiveDefinitionChooserId('')
    setDefinitionOptionQuery('')
    setShowDiff(true)
  }

  async function handlePdfFileChange(event) {
    const [file] = event.target.files || []
    if (!file) {
      return
    }
    await handlePdfUpload(file)
    event.target.value = ''
  }

  async function handlePdfUpload(file) {
    setIsUploadingPdf(true)
    setError('')
    setWarnings([])
    setAddedAbbreviations([])
    setProposedDefinitions([])
    setOutputDefinitions([])

    try {
      const formData = new FormData()
      formData.append('file', file)
      formData.append('include_string_definitions', String(includeStringDefinitions))
      formData.append('use_conferences_bib', String(useConferencesBib))
      formData.append('rewrite_reference_key', String(rewriteReferenceKey))
      formData.append('keep_doi', String(keepDoi))

      const response = await fetch(withBasePath('api/lookup-pdf'), {
        method: 'POST',
        body: formData,
      })
      const payload = await readJson(response)
      if (!response.ok) {
        throw new Error(payload.error || 'PDF lookup failed.')
      }

      applyFormatPayload(payload, {
        setOutput,
        setEntries,
        setBaseEntries,
        setWarnings,
        setAddedAbbreviations,
        setProposedDefinitions,
        setOutputDefinitions,
        setBaseOutputDefinitions,
        setEntryCount,
        setFormattedCount,
        setUntouchedCount,
        setExpandedUntouched,
        setExpandedFullEntries,
        includeStringDefinitions,
      })
      if (payload.retrieved_input) {
        setInput(payload.retrieved_input)
      }
      setLoadedFilename('')
      setLookupDoiLabel(payload.normalized_doi || '')
      setLookupRfcLabel('')
      setPdfResolutionLabel(
        payload.normalized_doi
          ? `Resolved from PDF via DOI ${payload.normalized_doi}.`
          : `Resolved from PDF metadata${file?.name ? ` (${file.name})` : ''}.`,
      )
      setInputMode('doi')
      setShowDiff(true)
    } catch (requestError) {
      setError(requestError.message)
    } finally {
      setIsUploadingPdf(false)
    }
  }

  async function handleDroppedFiles(fileList) {
    if (inputMode === 'rfc') {
      setIsDragActive(false)
      return
    }

    const files = Array.from(fileList || [])
    setIsDragActive(false)

    if (files.length === 0) {
      return
    }
    if (files.length > 1) {
      setError('Please drop exactly one file at a time.')
      return
    }

    const [file] = files
    const lowerName = file.name.toLowerCase()
    if (lowerName.endsWith('.bib')) {
      const text = await file.text()
      setInput(text)
      setLoadedFilename(file.name)
      setLookupDoiLabel('')
      setLookupRfcLabel('')
      setPdfResolutionLabel('')
      setInputMode('paste')
      setError('')
      return
    }

    if (lowerName.endsWith('.pdf') || file.type === 'application/pdf') {
      await handlePdfUpload(file)
      return
    }

    setError('Only .bib and .pdf files can be dropped here.')
  }

  function handleDragOver(event) {
    if (inputMode === 'rfc') {
      return
    }
    event.preventDefault()
    setIsDragActive(true)
  }

  function handleDragLeave(event) {
    if (inputMode === 'rfc') {
      return
    }
    event.preventDefault()
    if (event.currentTarget.contains(event.relatedTarget)) {
      return
    }
    setIsDragActive(false)
  }

  function handleDrop(event) {
    if (inputMode === 'rfc') {
      return
    }
    event.preventDefault()
    handleDroppedFiles(event.dataTransfer.files)
  }

  function toggleUntouched(index) {
    setExpandedUntouched((current) => ({
      ...current,
      [index]: !current[index],
    }))
  }

  function toggleFullEntry(index) {
    setExpandedFullEntries((current) => ({
      ...current,
      [index]: !current[index],
    }))
  }

  function handleDefinitionDraftChange(definitionId, field, value) {
    setProposedDefinitions((currentDefinitions) => {
      const currentDefinition = currentDefinitions.find((definition) => definition.id === definitionId)
      if (!currentDefinition) {
        return currentDefinitions
      }

      const nextDefinitions = currentDefinitions.map((definition) => {
        if (definition.id !== definitionId) {
          return definition
        }

        const nextToken = field === 'token' ? value : definition.token
        const currentCanonicalRawValue = buildCanonicalDefinitionRawValue(
          definition.expanded_value,
          definition.token,
        )
        const nextRawValue =
          field === 'raw_value'
            ? value
            : field === 'token' && definition.raw_value.trim() === currentCanonicalRawValue
              ? buildCanonicalDefinitionRawValue(definition.expanded_value, nextToken)
              : definition.raw_value
        const validationError = validateDefinitionDraft(nextToken, nextRawValue)

        return {
          ...definition,
          token: nextToken,
          raw_value: nextRawValue,
          expanded_value: extractExpandedValue(nextRawValue),
          definition_text: buildDefinitionText(nextToken, nextRawValue),
          validationError,
        }
      })

      const { entries: updatedEntries, outputDefinitions: nextOutputDefinitions } =
        deriveResolvedOutputState(baseEntries, baseOutputDefinitions, nextDefinitions)
      setOutputDefinitions(nextOutputDefinitions)
      setEntries(updatedEntries)
      setOutput(rebuildOutputDocument(updatedEntries, nextOutputDefinitions, includeStringDefinitions))
      return nextDefinitions
    })
  }

  function toggleDefinitionEditing(definitionId) {
    setProposedDefinitions((currentDefinitions) =>
      currentDefinitions.map((definition) =>
        definition.id === definitionId
          ? { ...definition, isEditing: !definition.isEditing }
          : definition,
      ),
    )
  }

  async function handleAddDefinition(definition) {
    if (!canManageAbbreviations) {
      setError(
        'Sign in as a kn-staff or kn-esslingen member to update conferences.bib.',
      )
      return
    }

    if (definition.validationError) {
      setError(definition.validationError)
      return
    }

    setSavingDefinitionId(definition.id)
    setError('')

    try {
      const response = await fetch(withBasePath('api/add-definition'), {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          token: definition.token,
          raw_value: definition.raw_value,
        }),
      })

      const payload = await readJson(response)
      if (!response.ok) {
        throw new Error(payload.error || 'Failed to update conferences.bib.')
      }

      setAddedAbbreviations((current) => {
        const next = [...current]
        if (!next.includes(payload.added_abbreviation)) {
          next.push(payload.added_abbreviation)
        }
        return next
      })
      setProposedDefinitions((current) => {
        const nextDefinitions = current.map((candidate) =>
          candidate.id === definition.id
            ? { ...candidate, resolution: 'added', isCollapsed: true }
            : candidate,
        )
        const { entries: updatedEntries, outputDefinitions: nextOutputDefinitions } =
          deriveResolvedOutputState(baseEntries, baseOutputDefinitions, nextDefinitions)
        setEntries(updatedEntries)
        setOutputDefinitions(nextOutputDefinitions)
        setOutput(
          rebuildOutputDocument(updatedEntries, nextOutputDefinitions, includeStringDefinitions),
        )
        return nextDefinitions
      })
    } catch (requestError) {
      setError(requestError.message)
    } finally {
      setSavingDefinitionId('')
    }
  }

  async function loadDefinitionOptions() {
    setDefinitionOptionsLoading(true)
    setDefinitionOptionsError('')
    try {
      const response = await fetch(withBasePath('api/definition-options'))
      const payload = await readJson(response)
      if (!response.ok) {
        throw new Error(payload.error || 'Failed to load abbreviation options.')
      }

      setDefinitionOptions(payload.definitions || [])
    } catch (requestError) {
      setDefinitionOptionsError(requestError.message)
    } finally {
      setDefinitionOptionsLoading(false)
    }
  }

  async function handleOpenDefinitionChooser(definitionId) {
    if (activeDefinitionChooserId === definitionId) {
      setActiveDefinitionChooserId('')
      setDefinitionOptionQuery('')
      setDefinitionOptionsError('')
      return
    }

    setActiveDefinitionChooserId(definitionId)
    setDefinitionOptionQuery('')
    if (definitionOptions.length === 0 && !definitionOptionsLoading) {
      await loadDefinitionOptions()
    }
  }

  function handleReopenDefinition(definitionId) {
    setActiveDefinitionChooserId('')
    setDefinitionOptionQuery('')
    setProposedDefinitions((currentDefinitions) =>
      currentDefinitions.map((definition) =>
        definition.id === definitionId
          ? { ...definition, isCollapsed: false }
          : definition,
      ),
    )
  }

  function handleRejectDefinition(definitionId) {
    setProposedDefinitions((currentDefinitions) => {
      const nextDefinitions = currentDefinitions.map((definition) =>
        definition.id === definitionId
          ? {
              ...definition,
              resolution: 'rejected',
              resolved_definition: null,
              isCollapsed: true,
            }
          : definition,
      )
      const { entries: updatedEntries, outputDefinitions: nextOutputDefinitions } =
        deriveResolvedOutputState(baseEntries, baseOutputDefinitions, nextDefinitions)
      setEntries(updatedEntries)
      setOutputDefinitions(nextOutputDefinitions)
      setActiveDefinitionChooserId('')
      setDefinitionOptionQuery('')
      setOutput(
        rebuildOutputDocument(updatedEntries, nextOutputDefinitions, includeStringDefinitions),
      )
      return nextDefinitions
    })
  }

  function handleUseExistingSuggestion(definitionId, suggestion) {
    setProposedDefinitions((currentDefinitions) => {
      const nextDefinitions = currentDefinitions.map((definition) =>
        definition.id === definitionId
          ? {
              ...definition,
              resolution: 'existing',
              resolved_definition: suggestion,
              isCollapsed: true,
            }
          : definition,
      )
      const { entries: updatedEntries, outputDefinitions: nextOutputDefinitions } =
        deriveResolvedOutputState(baseEntries, baseOutputDefinitions, nextDefinitions)
      setEntries(updatedEntries)
      setOutputDefinitions(nextOutputDefinitions)
      setActiveDefinitionChooserId('')
      setDefinitionOptionQuery('')
      setOutput(
        rebuildOutputDocument(updatedEntries, nextOutputDefinitions, includeStringDefinitions),
      )
      return nextDefinitions
    })
  }

  function handleUseExistingDefinition(definitionId, definitionOption) {
    handleUseExistingSuggestion(definitionId, definitionOption)
    setActiveDefinitionChooserId('')
    setDefinitionOptionQuery('')
  }

  return (
    <main className="app-shell">
      <section className="hero-panel shadow-lg">
        <div className="hero-toolbar">
          {authStatus.loaded && authStatus.enabled ? (
            authStatus.authenticated ? (
              <>
                <a
                  className="btn btn-outline-secondary btn-sm auth-button"
                  href={withBasePath('app')}
                >
                  Open library
                </a>
                {canManageAbbreviations ? (
                  <button
                    type="button"
                    className="btn btn-outline-secondary btn-sm auth-button"
                    onClick={openDefinitionManager}
                  >
                    Manage abbreviations
                  </button>
                ) : null}
                <div className="auth-pill">
                  <span className="auth-pill-label">{authStatus.email}</span>
                  <button
                    type="button"
                    className="btn btn-outline-secondary btn-sm"
                    onClick={handleLogout}
                  >
                    Sign out
                  </button>
                </div>
              </>
            ) : (
              <LoginProviderPopup
                providers={authStatus.providers}
                onSelectProvider={handleLoginProvider}
                buttonClassName="btn btn-outline-secondary btn-sm auth-button"
              />
            )
          ) : null}
          <button
            type="button"
            className="helper-toggle"
            onClick={() => setShowHelper(true)}
            aria-label="Show formatting rules"
            title="Formatting rules"
          >
            ?
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

        <div className="brand-row">
          <img
            src={withBasePath('logo_clean.png')}
            width="84"
            height="84"
            alt="menthTex logo"
            className="brand-logo"
          />
          <div>
            <p className="eyebrow">BibTeX formatter</p>
            <h1>MenthBibTex Formatter</h1>
          </div>
        </div>
        <p className="hero-text">
          Paste BibTeX or fetch a DOI, then export a cleaned entry in one pass.
        </p>
      </section>

      {authFlash ? (
        <div className={`auth-toast auth-toast-${authFlash.type}`} role="status" aria-live="polite">
          <span>{authFlash.message}</span>
          <button
            type="button"
            className="auth-toast-close"
            onClick={() => setAuthFlash(null)}
            aria-label="Dismiss notification"
          >
            ×
          </button>
        </div>
      ) : null}

      {showHelper ? (
        <div className="helper-overlay" role="presentation" onClick={() => setShowHelper(false)}>
          <section
            className="helper-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="format-contract-title"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="helper-header">
              <div>
                <p className="panel-kicker">Helper</p>
                <h2 id="format-contract-title">Formatting contract</h2>
              </div>
              <button
                type="button"
                className="btn btn-outline-secondary btn-sm"
                onClick={() => setShowHelper(false)}
              >
                Close
              </button>
            </div>
            <ul className="helper-list">
              {FORMAT_CONTRACT.map((rule) => (
                <li key={rule}>{rule}</li>
              ))}
            </ul>
          </section>
        </div>
      ) : null}

      {showDefinitionManager ? (
        <div
          className="helper-overlay"
          role="presentation"
          onClick={() => setShowDefinitionManager(false)}
        >
          <section
            className="helper-modal definition-manager-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="definition-manager-title"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="helper-header">
              <div>
                <p className="panel-kicker">Definitions</p>
                <h2 id="definition-manager-title">Manage conferences.bib</h2>
              </div>
              <button
                type="button"
                className="btn btn-outline-secondary btn-sm"
                onClick={() => setShowDefinitionManager(false)}
              >
                Close
              </button>
            </div>

            <div className="definition-manager-toolbar">
              <input
                type="search"
                className="form-control"
                value={definitionSearch}
                onChange={(event) => setDefinitionSearch(event.target.value)}
                placeholder="Search by token or long name"
              />
              <button
                type="button"
                className="btn btn-outline-secondary"
                onClick={loadManagedDefinitions}
                disabled={definitionsLoading}
              >
                {definitionsLoading ? 'Refreshing...' : 'Refresh'}
              </button>
            </div>

            <div className="definition-manager-create">
              <h3>Add definition</h3>
              <div className="definition-manager-grid">
                <label className="definition-field">
                  <span className="field-label">Token</span>
                  <input
                    type="text"
                    className="form-control"
                    value={newManagedDefinition.token}
                    onChange={(event) => handleNewDefinitionFieldChange('token', event.target.value)}
                  />
                </label>
                <label className="definition-field">
                  <span className="field-label">Long name</span>
                  <input
                    type="text"
                    className="form-control"
                    value={newManagedDefinition.longName}
                    onChange={(event) => handleNewDefinitionFieldChange('longName', event.target.value)}
                  />
                </label>
              </div>
              <pre className="definition-preview definition-manager-preview">
                {newManagedDefinition.definition_text || '@STRING{TOKEN = {{Long Name (TOKEN)}}}'}
              </pre>
              <div className="definition-manager-actions">
                {newManagedDefinition.validationError ? (
                  <p className="definition-validation-error">{newManagedDefinition.validationError}</p>
                ) : (
                  <p className="helper-text">New entries are stored in canonical formatter style.</p>
                )}
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={handleCreateManagedDefinition}
                  disabled={definitionMutationId === newManagedDefinition.id || Boolean(newManagedDefinition.validationError)}
                >
                  {definitionMutationId === newManagedDefinition.id ? 'Adding...' : 'Add definition'}
                </button>
              </div>
            </div>

            {definitionManagerError ? (
              <div className="alert alert-danger mb-0">{definitionManagerError}</div>
            ) : null}

            <div className="definition-manager-list">
              {definitionsLoading ? (
                <div className="empty-state compact-empty-state">Loading definitions...</div>
              ) : filteredManagedDefinitions.length === 0 ? (
                <div className="empty-state compact-empty-state">No definitions match the current search.</div>
              ) : (
                filteredManagedDefinitions.map((definition) => (
                  <article className="definition-card" key={definition.id}>
                    <div className="definition-card-header">
                      <div>
                        <strong>{definition.token}</strong>
                        <p className="helper-text">{definition.longName}</p>
                      </div>
                      <div className="definition-card-actions">
                        <button
                          type="button"
                          className="btn btn-outline-secondary btn-sm"
                          onClick={() => toggleManagedDefinitionEditing(definition.id)}
                        >
                          {definition.isEditing ? 'Cancel' : 'Edit'}
                        </button>
                        <button
                          type="button"
                          className="btn btn-outline-danger btn-sm"
                          onClick={() => handleDeleteManagedDefinition(definition)}
                          disabled={definitionMutationId === definition.id}
                        >
                          {definitionMutationId === definition.id ? 'Deleting...' : 'Delete'}
                        </button>
                      </div>
                    </div>
                    {definition.isEditing ? (
                      <div className="definition-edit-grid">
                        <label className="definition-field">
                          <span className="field-label">Token</span>
                          <input
                            type="text"
                            className="form-control"
                            value={definition.token}
                            onChange={(event) =>
                              handleManagedDefinitionFieldChange(definition.id, 'token', event.target.value)
                            }
                          />
                        </label>
                        <label className="definition-field">
                          <span className="field-label">Long name</span>
                          <input
                            type="text"
                            className="form-control"
                            value={definition.longName}
                            onChange={(event) =>
                              handleManagedDefinitionFieldChange(definition.id, 'longName', event.target.value)
                            }
                          />
                        </label>
                        <pre className="definition-preview definition-manager-preview">
                          {definition.definition_text}
                        </pre>
                        <div className="definition-manager-actions">
                          {definition.validationError ? (
                            <p className="definition-validation-error">{definition.validationError}</p>
                          ) : (
                            <p className="helper-text">Saving rewrites this entry in canonical style.</p>
                          )}
                          <button
                            type="button"
                            className="btn btn-primary btn-sm"
                            onClick={() => handleSaveManagedDefinition(definition)}
                            disabled={definitionMutationId === definition.id || Boolean(definition.validationError)}
                          >
                            {definitionMutationId === definition.id ? 'Saving...' : 'Save changes'}
                          </button>
                        </div>
                      </div>
                    ) : (
                      <pre className="definition-preview definition-manager-preview">
                        {definition.definition_text}
                      </pre>
                    )}
                  </article>
                ))
              )}
            </div>
          </section>
        </div>
      ) : null}

      <form className="formatter-grid" onSubmit={handleSubmit}>
        <section
          className={`panel-card input-panel ${isDragActive ? 'input-panel-dragging' : ''} ${isUploadingPdf ? 'input-panel-busy' : ''}`}
          onDragOver={inputMode === 'rfc' ? undefined : handleDragOver}
          onDragLeave={inputMode === 'rfc' ? undefined : handleDragLeave}
          onDrop={inputMode === 'rfc' ? undefined : handleDrop}
        >
          <div className="panel-header">
            <div className="panel-header-spread">
              <div>
                <p className="panel-kicker">Input</p>
                <h2>
                  {inputMode === 'doi'
                    ? 'Import From DOI or PDF'
                    : inputMode === 'rfc'
                      ? 'Import RFC'
                      : 'Paste BibTeX'}
                </h2>
              </div>
              <div className="panel-action-toolbar">
                <button
                  type="button"
                  className="btn btn-outline-secondary icon-action-button"
                  onClick={handleClearInput}
                  disabled={
                    !input
                    && !doiInput
                    && !rfcInput
                    && !loadedFilename
                    && !lookupDoiLabel
                    && !lookupRfcLabel
                    && !pdfResolutionLabel
                  }
                  title="Clear input"
                  aria-label="Clear input"
                >
                  <TrashIcon />
                </button>
              </div>
            </div>
            <div className="mode-switch" role="tablist" aria-label="Input mode">
              <button
                type="button"
                className={`mode-pill ${inputMode === 'paste' ? 'mode-pill-active' : ''}`}
                onClick={() => setInputMode('paste')}
              >
                BibTeX
              </button>
              <button
                type="button"
                className={`mode-pill ${inputMode === 'doi' ? 'mode-pill-active' : ''}`}
                onClick={() => setInputMode('doi')}
              >
                DOI or PDF
              </button>
              <button
                type="button"
                className={`mode-pill ${inputMode === 'rfc' ? 'mode-pill-active' : ''}`}
                onClick={() => setInputMode('rfc')}
              >
                RFC
              </button>
            </div>
            <div className="input-controls">
              <details className="input-options">
                <summary>Formatting options</summary>
                <div className="input-options-grid">
                  <label className="toggle-row">
                    <input
                      type="checkbox"
                      className="form-check-input"
                      checked={includeStringDefinitions}
                      onChange={(event) => setIncludeStringDefinitions(event.target.checked)}
                    />
                    <span>Include full <code>@STRING</code> definitions</span>
                  </label>
                  <label className="toggle-row">
                    <input
                      type="checkbox"
                      className="form-check-input"
                      checked={useConferencesBib}
                      onChange={(event) => setUseConferencesBib(event.target.checked)}
                    />
                    <span>Use <code>conferences.bib</code> for abbreviations</span>
                  </label>
                  <label className="toggle-row">
                    <input
                      type="checkbox"
                      className="form-check-input"
                      checked={!rewriteReferenceKey}
                      onChange={(event) => setRewriteReferenceKey(!event.target.checked)}
                    />
                    <span>Keep reference key</span>
                  </label>
                  <label className="toggle-row">
                    <input
                      type="checkbox"
                      className="form-check-input"
                      checked={keepDoi}
                      onChange={(event) => setKeepDoi(event.target.checked)}
                    />
                    <span>Keep DOI</span>
                  </label>
                </div>
              </details>
            </div>
            {inputMode === 'doi' ? (
              <div className="doi-fetch-row">
                <input
                  type="text"
                  className="form-control doi-input"
                  value={doiInput}
                  onChange={(event) => setDoiInput(event.target.value)}
                  placeholder="DOI or DOI URL"
                  aria-label="DOI lookup input"
                />
              </div>
            ) : inputMode === 'rfc' ? (
              <div className="doi-fetch-row">
                <input
                  type="text"
                  className="form-control doi-input"
                  value={rfcInput}
                  onChange={(event) => setRfcInput(event.target.value)}
                  placeholder="RFC1034 or 1034"
                  aria-label="RFC lookup input"
                />
              </div>
            ) : null}
          </div>

          {inputMode !== 'rfc' ? (
            <>
              <button
                type="button"
                className={`dropzone-hint ${isDragActive ? 'dropzone-hint-active' : ''}`}
                onClick={openDropzonePicker}
                disabled={isUploadingPdf}
              >
                {inputMode === 'doi' ? (
                  <>
                    Drop or click to upload one <code>.pdf</code> file.
                  </>
                ) : (
                  <>
                    Drop or click to upload one <code>.bib</code> file.
                  </>
                )}
              </button>
              <input
                ref={dropzoneInputRef}
                type="file"
                accept={inputMode === 'doi' ? '.pdf,application/pdf' : '.bib,text/plain'}
                onChange={handleDropzoneFileChange}
                className="visually-hidden"
              />
            </>
          ) : null}

          {isUploadingPdf ? (
            <div className="upload-status" role="status" aria-live="polite">
              <span className="upload-spinner" aria-hidden="true" />
              <div>
                <strong>Uploading PDF and extracting metadata…</strong>
                <p className="helper-text mb-0">
                  This can take a few seconds depending on file size and server load.
                </p>
              </div>
            </div>
          ) : null}

          {inputMode === 'paste' ? (
            <textarea
              className="form-control editor"
              value={input}
              onChange={(event) => setInput(event.target.value)}
              spellCheck="false"
              aria-label="BibTeX input"
            />
          ) : input ? (
            <details className="raw-preview">
              <summary>Show retrieved raw BibTeX</summary>
              <textarea
                className="form-control editor editor-compact"
                value={input}
                onChange={(event) => setInput(event.target.value)}
                spellCheck="false"
                aria-label="Retrieved BibTeX input"
              />
            </details>
          ) : (
            <div className="empty-state compact-empty-state">
              {inputMode === 'rfc'
                ? 'Enter an RFC number to fetch a source entry, then review or reformat it here.'
                : 'Enter a DOI or upload a PDF to fetch a source entry, then review or reformat it here.'}
            </div>
          )}

          <div className="panel-footer">
            <p className="helper-text">
              {loadedFilename
                ? `Loaded ${loadedFilename}.`
                : pdfResolutionLabel
                  ? pdfResolutionLabel
                : lookupRfcLabel
                  ? `Fetched RFC ${lookupRfcLabel}.`
                : lookupDoiLabel
                  ? `Fetched DOI ${lookupDoiLabel}.`
                  : inputMode === 'doi'
                    ? 'Enter a DOI or upload a PDF, then use the red button to fetch and format it in one step.'
                    : inputMode === 'rfc'
                      ? 'Enter an RFC number, then use the red button to fetch and format it in one step.'
                    : 'Paste or upload a single entry or a full .bib file.'}
            </p>
            <button type="submit" className="btn btn-primary btn-lg" disabled={isSubmitting}>
              {isSubmitting
                ? 'Formatting...'
                : inputMode === 'paste'
                  ? 'Format document'
                  : inputMode === 'doi'
                    ? 'Fetch and format'
                    : 'Fetch and format RFC'}
            </button>
          </div>
        </section>

        <section className="panel-card output-panel">
          <div className="panel-header panel-header-spread">
            <div>
              <p className="panel-kicker">Output</p>
              <h2>Final document</h2>
              {entryCount > 0 ? (
                <p className="result-summary">
                  {formattedCount} formatted, {untouchedCount} untouched, {entryCount} total
                </p>
              ) : null}
            </div>
            <div className="output-actions panel-action-toolbar">
              <button
                type="button"
                className={`btn btn-outline-secondary icon-action-button ${copyLabel === 'Copied' ? 'icon-action-button-active' : ''}`}
                onClick={handleCopy}
                disabled={!output}
                title={copyLabel}
                aria-label={copyLabel}
              >
                <CopyIcon />
              </button>
              <button
                type="button"
                className="btn btn-outline-secondary icon-action-button"
                onClick={handleDownload}
                disabled={!output}
                title="Download output"
                aria-label="Download output"
              >
                <DownloadIcon />
              </button>
              <button
                type="button"
                className="btn btn-outline-secondary icon-action-button"
                onClick={handleClearOutput}
                disabled={!output}
                title="Clear output"
                aria-label="Clear output"
              >
                <TrashIcon />
              </button>
            </div>
          </div>

          <textarea
            className="form-control editor editor-output"
            value={output}
            readOnly
            spellCheck="false"
            aria-label="Formatted BibTeX output"
            placeholder="Formatted output appears here."
          />

          <div className="feedback-stack">
            {error ? <div className="alert alert-danger mb-0">{error}</div> : null}

            {proposedDefinitions.length > 0 ? (
              <details className="definition-review">
                <summary className="definition-review-summary">
                  <span className="definition-review-copy">
                    <span className="definition-review-title">
                      Review {proposedDefinitions.length} new {proposedDefinitions.length === 1 ? 'abbreviation' : 'abbreviations'}
                    </span>
                    <span className="helper-text">Verify before adding to <code>conferences.bib</code>.</span>
                  </span>
                  <span className="definition-review-indicator" aria-hidden="true">Expand</span>
                </summary>
                <div className="definition-review-list">
                  {!authStatus.enabled ? (
                    <div className="alert alert-info mb-0">
                      Authentication is not configured on this deployment. Adding to <code>conferences.bib</code> is disabled.
                    </div>
                  ) : !canManageAbbreviations ? (
                    <div className="definition-auth-callout">
                      <p className="helper-text">
                        Sign in as a <code>kn-staff</code> or <code>kn-esslingen</code> member to add reviewed abbreviations to <code>conferences.bib</code>.
                      </p>
                      <LoginProviderPopup
                        providers={authStatus.providers}
                        onSelectProvider={handleLoginProvider}
                      />
                    </div>
                  ) : null}
                  {proposedDefinitions.map((definition) => {
                    const resolutionMeta = definitionResolutionMeta(definition)
                    const isCollapsedResolved =
                      isDefinitionResolved(definition) && definition.isCollapsed

                    return (
                    <article className={`definition-card ${resolutionMeta.className} ${isCollapsedResolved ? 'definition-card-collapsed' : ''}`} key={definition.id}>
                      <div className="definition-card-header">
                        <div className={isCollapsedResolved ? 'definition-card-summary' : undefined}>
                          {isCollapsedResolved ? (
                            <span className="definition-resolved-check" aria-hidden="true">
                              <CheckIcon />
                            </span>
                          ) : null}
                          <div>
                            <strong>{definition.token}</strong>
                            <p className="helper-text">{definition.expanded_value}</p>
                            <span className="definition-resolution-chip">
                              {resolutionMeta.label}
                            </span>
                          </div>
                        </div>
                        <div className="definition-card-actions">
                          {isCollapsedResolved ? (
                            <button
                              type="button"
                              className="btn btn-outline-secondary btn-sm icon-button"
                              onClick={() => handleReopenDefinition(definition.id)}
                              title="Change resolution"
                              aria-label="Change resolution"
                            >
                              <PenIcon />
                            </button>
                          ) : (
                            <>
                              <button
                                type="button"
                                className="btn btn-outline-secondary btn-sm icon-button"
                                onClick={() => toggleDefinitionEditing(definition.id)}
                                title="Edit proposed definition"
                                aria-label="Edit proposed definition"
                              >
                                <PenIcon />
                              </button>
                              <button
                                type="button"
                                className="btn btn-outline-secondary btn-sm icon-button"
                                onClick={() => handleRejectDefinition(definition.id)}
                                title="Reject abbreviation and keep the literal venue name"
                                aria-label="Reject abbreviation and keep the literal venue name"
                              >
                                <RejectIcon />
                              </button>
                              <button
                                type="button"
                                className={`btn btn-outline-secondary btn-sm icon-button ${activeDefinitionChooserId === definition.id ? 'icon-button-active' : ''}`}
                                onClick={() => handleOpenDefinitionChooser(definition.id)}
                                title="Choose another existing abbreviation"
                                aria-label="Choose another existing abbreviation"
                              >
                                <BrowseIcon />
                              </button>
                              <button
                                type="button"
                                className="btn btn-outline-secondary btn-sm icon-button"
                                onClick={() => handleAddDefinition(definition)}
                                disabled={
                                  savingDefinitionId === definition.id
                                  || Boolean(definition.validationError)
                                  || !canManageAbbreviations
                                }
                                title={
                                  savingDefinitionId === definition.id
                                    ? 'Adding abbreviation to conferences.bib'
                                    : 'Add reviewed abbreviation to conferences.bib'
                                }
                                aria-label={
                                  savingDefinitionId === definition.id
                                    ? 'Adding abbreviation to conferences.bib'
                                    : 'Add reviewed abbreviation to conferences.bib'
                                }
                              >
                                <AddIcon />
                              </button>
                            </>
                          )}
                        </div>
                      </div>
                      {!isCollapsedResolved ? (
                        <>
                      {definition.suggestions?.length > 0 ? (
                        <div className="definition-suggestion-list">
                          <span className="field-label">Similar existing venues</span>
                          {definition.suggestions.map((suggestion) => (
                            <div
                              className="definition-suggestion"
                              key={`${definition.id}-${suggestion.token}`}
                            >
                              <div>
                                <strong>{suggestion.token}</strong>
                                <p className="helper-text">
                                  {suggestion.expanded_value}
                                  {typeof suggestion.score === 'number'
                                    ? ` · ${suggestion.score}% match`
                                    : ''}
                                </p>
                              </div>
                              <button
                                type="button"
                                className="btn btn-outline-secondary btn-sm"
                                onClick={() => handleUseExistingSuggestion(definition.id, suggestion)}
                              >
                                Use existing
                              </button>
                            </div>
                          ))}
                        </div>
                      ) : null}
                      {activeDefinitionChooserId === definition.id ? (
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
                                .filter((option) => option.token.toUpperCase() !== definition.token.toUpperCase())
                                .filter((option) => {
                                  const query = definitionOptionQuery.trim().toLowerCase()
                                  if (!query) {
                                    return true
                                  }

                                  return (
                                    option.token.toLowerCase().includes(query)
                                    || option.expanded_value.toLowerCase().includes(query)
                                  )
                                })
                                .slice(0, 12)

                              return filteredOptions.length > 0 ? (
                                <div className="definition-chooser-list">
                                  {filteredOptions.map((option) => (
                                    <button
                                      type="button"
                                      className="definition-chooser-option"
                                      key={`${definition.id}-${option.token}`}
                                      onClick={() => handleUseExistingDefinition(definition.id, option)}
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
                      {definition.isEditing ? (
                        <div className="definition-edit-grid">
                          <label className="definition-field">
                            <span className="field-label">Token</span>
                            <input
                              type="text"
                              className="form-control"
                              value={definition.token}
                              onChange={(event) =>
                                handleDefinitionDraftChange(definition.id, 'token', event.target.value)
                              }
                            />
                          </label>
                          <label className="definition-field">
                            <span className="field-label">Value</span>
                            <textarea
                              className="form-control definition-value-input"
                              value={definition.raw_value}
                              onChange={(event) =>
                                handleDefinitionDraftChange(definition.id, 'raw_value', event.target.value)
                              }
                              spellCheck="false"
                            />
                          </label>
                          {definition.validationError ? (
                            <p className="definition-validation-error">{definition.validationError}</p>
                          ) : (
                            <p className="helper-text">
                              Valid preview. Saving will append this exact entry to <code>conferences.bib</code>.
                            </p>
                          )}
                        </div>
                      ) : null}
                      <pre className="definition-preview">{definition.definition_text}</pre>
                        </>
                      ) : null}
                    </article>
                    )
                  })}
                </div>
              </details>
            ) : null}

            {warnings.length > 0 ? (
              <div className="alert alert-warning mb-0">
                <strong>Warnings</strong>
                <p className="warning-headline">{warningHeadline}</p>
                <details className="warning-details">
                  <summary>Show details</summary>
                  <ul className="feedback-list feedback-list-compact">
                    {warningSummaries.map((warning) => (
                      <li key={warning}>{warning}</li>
                    ))}
                  </ul>
                  {warnings.length !== warningSummaries.length ? (
                    <ul className="feedback-list feedback-list-compact">
                      {warnings.map((warning) => (
                        <li key={warning}>{warning}</li>
                      ))}
                    </ul>
                  ) : null}
                </details>
              </div>
            ) : null}

            {addedAbbreviations.length > 0 ? (
              <div className="alert alert-info mb-0">
                <strong>Added to <code>conferences.bib</code></strong>
                <ul className="feedback-list">
                  {addedAbbreviations.map((abbreviation) => (
                    <li key={abbreviation}>{abbreviation}</li>
                  ))}
                </ul>
              </div>
            ) : null}
          </div>
        </section>
      </form>

      <section className="diff-panel">
        <div className="diff-header">
          <div>
            <p className="panel-kicker">Diff</p>
            <h2>Per-entry changes</h2>
          </div>
          <div className="diff-actions">
            {diffEntries.length > 0 ? (
              <span className="diff-caption">{diffEntries.length} changed {diffEntries.length === 1 ? 'entry' : 'entries'}</span>
            ) : null}
            {diffEntries.length > 0 ? (
              <button
                type="button"
                className="btn btn-outline-secondary btn-sm"
                onClick={() => setShowDiff((current) => !current)}
              >
                {showDiff ? 'Hide changes' : 'Show changes'}
              </button>
            ) : null}
          </div>
        </div>
        <p className="diff-disclaimer">
          The formatter still makes best-effort guesses. Double-check the diff before using the result.
        </p>

        {diffEntries.length === 0 ? (
          <div className="empty-state">
            Run the formatter to inspect removed fields and normalized fields entry by entry.
          </div>
        ) : !showDiff ? (
          <div className="empty-state compact-empty-state">
            Changes are available, but hidden by default to keep the page focused on input and output.
          </div>
        ) : (
          <div className="diff-stack">
            {diffEntries.map((entry, index) => {
              const untouched = entry.status === 'untouched'
              const isExpanded = expandedUntouched[index]
              const showFullEntry = expandedFullEntries[index]
              const diffResult = untouched
                ? { rows: [], suppressedNormalizations: [] }
                : buildLineDiff(entry.original_text, entry.output_text)
              const diffRows = diffResult.rows
              const displayNormalizedFields = [
                ...(entry.normalized_fields || []),
                ...(diffResult.suppressedNormalizations || []),
              ].filter((field, fieldIndex, fields) => fields.indexOf(field) === fieldIndex)
              return (
                <article className="diff-card" key={`${entry.entry_type}-${entry.key}-${index}`}>
                  <div className="diff-card-header">
                    <div>
                      <div className="diff-title-row">
                        <span className={`status-chip ${untouched ? 'status-muted' : 'status-active'}`}>
                          {entry.status}
                        </span>
                        <span className="entry-meta">
                          @{entry.entry_type} {entry.key ? `· ${entry.key}` : ''}
                        </span>
                      </div>
                      {!untouched ? (
                        <div className="change-groups">
                          {displayNormalizedFields.length > 0 ? (
                            <div className="change-row">
                              <span className="change-label">Normalized</span>
                              <div className="chip-group">
                                {displayNormalizedFields.map((field) => (
                                  <span className="field-chip" key={`${entry.key}-${field}-normalized`}>
                                    {field}
                                  </span>
                                ))}
                              </div>
                            </div>
                          ) : null}

                          {entry.removed_fields?.length > 0 ? (
                            <div className="change-row">
                              <span className="change-label">Removed</span>
                              <div className="chip-group">
                                {entry.removed_fields.map((field) => (
                                  <span className="field-chip field-chip-removed" key={`${entry.key}-${field}-removed`}>
                                    {field}
                                  </span>
                                ))}
                              </div>
                            </div>
                          ) : null}
                        </div>
                      ) : (
                        <p className="helper-text">Untouched because this entry type is not normalized yet.</p>
                      )}
                    </div>

                    <div className="diff-card-actions">
                      {!untouched ? (
                        <button
                          type="button"
                          className="btn btn-sm btn-outline-secondary"
                          onClick={() => toggleFullEntry(index)}
                        >
                          {showFullEntry ? 'Hide full entry' : 'Show full entry'}
                        </button>
                      ) : null}
                      {untouched ? (
                        <button
                          type="button"
                          className="btn btn-sm btn-outline-secondary"
                          onClick={() => toggleUntouched(index)}
                        >
                          {isExpanded ? 'Hide entry' : 'Show entry'}
                        </button>
                      ) : null}
                    </div>
                  </div>

                  {!untouched || isExpanded ? (
                    <div className="diff-columns">
                      {untouched ? (
                        <>
                          <div className="diff-column">
                            <p className="diff-column-label">Original</p>
                          </div>
                          <div className="diff-column">
                            <p className="diff-column-label">Untouched</p>
                          </div>
                          <pre className="diff-pane">{entry.original_text}</pre>
                          <pre className="diff-pane">{entry.output_text}</pre>
                        </>
                      ) : (
                        <div className="unified-diff">
                          <p className="diff-column-label">Patch</p>
                          <div className="diff-hunk">
                            {diffRows.length === 0 ? (
                              <div className="diff-line diff-line-unchanged">
                                <span className="diff-gutter"> </span>
                                <span className="diff-text">No textual line changes.</span>
                              </div>
                            ) : (
                              diffRows.map((row, rowIndex) => (
                                <div className={`diff-line diff-line-${row.type}`} key={`${entry.key}-${rowIndex}`}>
                                  <span className="diff-gutter">
                                    {row.type === 'removed' ? '-' : '+'}
                                  </span>
                                  <span className="diff-text">
                                    {row.type === 'removed' ? row.left : row.right}
                                  </span>
                                </div>
                              ))
                            )}
                          </div>
                        </div>
                      )}

                      {!untouched && showFullEntry ? (
                        <>
                          <div className="diff-column">
                            <p className="diff-column-label">Original entry</p>
                            <pre className="diff-pane">{entry.original_text}</pre>
                          </div>
                          <div className="diff-column">
                            <p className="diff-column-label">Normalized entry</p>
                            <pre className="diff-pane">{entry.output_text}</pre>
                          </div>
                        </>
                      ) : null}
                    </div>
                  ) : null}
                </article>
              )
            })}
          </div>
        )}
      </section>

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
    </main>
  )
}

function App() {
  const [routePath, setRoutePath] = useState(currentRelativePath)

  useEffect(() => {
    function handleLocationChange() {
      setRoutePath(currentRelativePath())
    }

    window.addEventListener('popstate', handleLocationChange)
    return () => window.removeEventListener('popstate', handleLocationChange)
  }, [])

  if (routePath.startsWith('/app')) {
    return <WorkspaceApp routePath={routePath} />
  }

  return <FormatterApp />
}

export default App
