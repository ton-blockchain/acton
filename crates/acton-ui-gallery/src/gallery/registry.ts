import {buttonGallery} from "./buttonGallery"
import {breadcrumbsGallery} from "./breadcrumbsGallery"
import {checkboxGallery} from "./checkboxGallery"
import {contentTabsGallery} from "./contentTabsGallery"
import {dataTableGallery} from "./dataTableGallery"
import {disclosureToggleGallery} from "./disclosureToggleGallery"
import {exitCodeChipGallery} from "./exitCodeChipGallery"
import {infoPopoverGallery} from "./infoPopoverGallery"
import {inlineActionsGallery} from "./inlineActionsGallery"
import {inlineButtonGallery} from "./inlineButtonGallery"
import {markdownTextGallery} from "./markdownTextGallery"
import {pillTabsGallery} from "./pillTabsGallery"
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
  disclosureToggleGallery,
  exitCodeChipGallery,
  contentTabsGallery,
  pillTabsGallery,
  markdownTextGallery,
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
