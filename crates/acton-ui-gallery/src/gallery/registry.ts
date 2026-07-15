import {buttonGallery} from "./buttonGallery"
import {breadcrumbsGallery} from "./breadcrumbsGallery"
import {checkboxGallery} from "./checkboxGallery"
import {contentTabsGallery} from "./contentTabsGallery"
import {contractChipGallery} from "./contractChipGallery"
import {dataTableGallery} from "./dataTableGallery"
import {disclosureToggleGallery} from "./disclosureToggleGallery"
import {exitCodeChipGallery} from "./exitCodeChipGallery"
import {highlightedCodeGallery} from "./highlightedCodeGallery"
import {infoPopoverGallery} from "./infoPopoverGallery"
import {inlineActionsGallery} from "./inlineActionsGallery"
import {inlineButtonGallery} from "./inlineButtonGallery"
import {markdownTextGallery} from "./markdownTextGallery"
import {modeViewerGallery} from "./modeViewerGallery"
import {opcodeChipGallery} from "./opcodeChipGallery"
import {pillTabsGallery} from "./pillTabsGallery"
import {parsedBodySectionGallery} from "./parsedBodySectionGallery"
import {parsedValueDiffViewGallery} from "./parsedValueDiffViewGallery"
import {parsedValueViewGallery} from "./parsedValueViewGallery"
import {popoverGallery} from "./popoverGallery"
import {rawDataBlockGallery} from "./rawDataBlockGallery"
import {skeletonGallery} from "./skeletonGallery"
import {themeSwitchGallery} from "./themeSwitchGallery"
import {toastGallery} from "./toastGallery"
import {tokensGallery} from "./tokensGallery"
import {visuallyGroupedNumberGallery} from "./visuallyGroupedNumberGallery"
import type {ComponentGallery} from "./types"

export const galleries = [
  tokensGallery,
  buttonGallery,
  breadcrumbsGallery,
  inlineButtonGallery,
  inlineActionsGallery,
  contractChipGallery,
  disclosureToggleGallery,
  exitCodeChipGallery,
  opcodeChipGallery,
  parsedValueViewGallery,
  parsedValueDiffViewGallery,
  parsedBodySectionGallery,
  highlightedCodeGallery,
  contentTabsGallery,
  pillTabsGallery,
  markdownTextGallery,
  modeViewerGallery,
  popoverGallery,
  infoPopoverGallery,
  toastGallery,
  rawDataBlockGallery,
  dataTableGallery,
  skeletonGallery,
  visuallyGroupedNumberGallery,
  checkboxGallery,
  themeSwitchGallery,
] satisfies readonly ComponentGallery[]
