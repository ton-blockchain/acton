import {useCallback, useMemo, useState} from "react"
import type {FC, ReactNode} from "react"
import {
  EyeOff,
  LogIn,
  LogOut,
  Play,
  Plus,
  Server,
  ShieldCheck,
  Square,
  Trash2,
  TriangleAlert,
} from "lucide-react"
import {Button, Checkbox, Dialog, DialogActions, InlineAction, Input, useToast} from "@acton/ui"
import {
  createObservabilityClient,
  NetworkDashboard,
  type NetworkDashboardView,
  type NetworkView,
  type NodeView,
} from "@acton/localton-ui"
import {useExplorerRoutePaths} from "@acton/explorer-core/hooks/useExplorerRoutePaths"
import {useOpenExplorerPath} from "@acton/explorer-core/hooks/useOpenExplorerPath"

import type {FullTonNode, StudioEnvironment} from "../../../studioApi"
import {
  addStudioFullTonNode,
  enterStudioFullTonValidation,
  leaveStudioFullTonValidation,
  removeStudioFullTonNode,
  setStudioFullTonNodeRunning,
} from "../../../studioApi"
import {useLocalnetRuntime} from "../../LocalnetRuntimeProvider"
import styles from "./NetworkPage.module.css"

interface NetworkPageProps {
  readonly onEnvironmentChange: (environment: StudioEnvironment) => void
  readonly view: NetworkDashboardView
}

/** Connects reusable Localton observability views to the selected Studio environment */
export const NetworkPage: FC<NetworkPageProps> = ({onEnvironmentChange, view}) => {
  const {environment} = useLocalnetRuntime()
  const explorerRoutes = useExplorerRoutePaths()
  const openExplorerPath = useOpenExplorerPath()
  const {showToast, updateToast} = useToast()
  const [network, setNetwork] = useState<NetworkView>()
  const [addDialogOpen, setAddDialogOpen] = useState(false)
  const [nodeName, setNodeName] = useState("")
  const [nodeIsValidator, setNodeIsValidator] = useState(true)
  const [isAdding, setIsAdding] = useState(false)
  const [removingNode, setRemovingNode] = useState<FullTonNode>()
  const [isRemoving, setIsRemoving] = useState(false)
  const [leavingNodeId, setLeavingNodeId] = useState<string>()
  const [enteringNodeId, setEnteringNodeId] = useState<string>()
  const [forgettingObserverId, setForgettingObserverId] = useState<string>()
  const [changingNodeId, setChangingNodeId] = useState<string>()
  const endpoint = environment?.endpoints.observability
  const config = environment?.config.kind === "fullTonNetwork" ? environment.config : undefined
  const client = useMemo(
    () => createObservabilityClient(endpoint ?? "/unavailable-observability"),
    [endpoint],
  )

  const managedNode = useCallback(
    (observed: NodeView) =>
      config?.nodes.find(node => node.name.toLowerCase() === observed.name.toLowerCase()),
    [config?.nodes],
  )

  const observedNode = useCallback(
    (node: FullTonNode) =>
      network?.nodes.find(candidate => candidate.name.toLowerCase() === node.name.toLowerCase()),
    [network?.nodes],
  )

  const addNode = useCallback(async () => {
    if (!environment || !config) return
    const name = nodeName.trim()
    if (!name) return

    setIsAdding(true)
    try {
      const updated = await addStudioFullTonNode(environment.id, {
        name,
        validator: nodeIsValidator,
      })
      onEnvironmentChange(updated)
      setNodeName("")
      setAddDialogOpen(false)
      showToast({
        variant: "success",
        title: "Node added",
        description: `${name} joined ${environment.name}`,
      })
    } catch (error) {
      showToast({
        variant: "error",
        title: "Node not added",
        description: message(error, "Failed to join the node"),
      })
    } finally {
      setIsAdding(false)
    }
  }, [config, environment, nodeIsValidator, nodeName, onEnvironmentChange, showToast])

  const setNodeRunning = useCallback(
    async (node: FullTonNode, running: boolean) => {
      if (!environment) return

      setChangingNodeId(node.id)
      const toastId = showToast({
        variant: "loading",
        title: running ? "Starting node" : "Stopping node",
        description: node.name,
        durationMs: 0,
      })
      try {
        const updated = await setStudioFullTonNodeRunning(environment.id, node.id, running)
        onEnvironmentChange(updated)
        updateToast(toastId, {
          variant: "success",
          title: running ? "Node started" : "Node stopped",
          description: node.name,
          durationMs: 4000,
        })
      } catch (error) {
        updateToast(toastId, {
          variant: "error",
          title: running ? "Node not started" : "Node not stopped",
          description: message(error, "Failed to change the node running state"),
          durationMs: 8000,
        })
      } finally {
        setChangingNodeId(undefined)
      }
    },
    [environment, onEnvironmentChange, showToast, updateToast],
  )

  const removeNode = useCallback(async () => {
    if (!environment || !removingNode) return

    setIsRemoving(true)
    try {
      const updated = await removeStudioFullTonNode(environment.id, removingNode.id)
      onEnvironmentChange(updated)
      showToast({
        variant: "success",
        title: "Node removed",
        description: `${removingNode.name} and its stored state were removed`,
      })
      setRemovingNode(undefined)
    } catch (error) {
      showToast({
        variant: "error",
        title: "Node not removed",
        description: message(error, "Failed to remove the node"),
      })
    } finally {
      setIsRemoving(false)
    }
  }, [environment, onEnvironmentChange, removingNode, showToast])

  const leaveValidation = useCallback(
    async (node: FullTonNode) => {
      if (!environment) return

      setLeavingNodeId(node.id)
      try {
        const updated = await leaveStudioFullTonValidation(environment.id, node.id)
        onEnvironmentChange(updated)
        setRemovingNode(current => (current?.id === node.id ? undefined : current))
        showToast({
          variant: "success",
          title: "Validator exit started",
          description: `${node.name} will stop participating after the current validator round`,
        })
      } catch (error) {
        showToast({
          variant: "error",
          title: "Validator exit not started",
          description: message(error, "Failed to disable validator participation"),
        })
      } finally {
        setLeavingNodeId(undefined)
      }
    },
    [environment, onEnvironmentChange, showToast],
  )

  const enterValidation = useCallback(
    async (node: FullTonNode) => {
      if (!environment) return

      setEnteringNodeId(node.id)
      try {
        const updated = await enterStudioFullTonValidation(environment.id, node.id)
        onEnvironmentChange(updated)
        showToast({
          variant: "success",
          title: "Validator entry started",
          description: `${node.name} will participate in upcoming elections`,
        })
      } catch (error) {
        showToast({
          variant: "error",
          title: "Validator entry not started",
          description: message(error, "Failed to enable validator participation"),
        })
      } finally {
        setEnteringNodeId(undefined)
      }
    },
    [environment, onEnvironmentChange, showToast],
  )

  const forgetNode = useCallback(
    async (node: NodeView) => {
      setForgettingObserverId(node.observer_id)
      try {
        await client.forget(node.observer_id)
        setNetwork(current =>
          current
            ? {
                ...current,
                nodes: current.nodes.filter(item => item.observer_id !== node.observer_id),
              }
            : current,
        )
        showToast({
          variant: "success",
          title: "Node forgotten",
          description: `${node.name} was removed from collector history`,
        })
      } catch (error) {
        showToast({
          variant: "error",
          title: "Node not forgotten",
          description: message(error, "Failed to forget the node"),
        })
      } finally {
        setForgettingObserverId(undefined)
      }
    },
    [client, showToast],
  )

  const removalObservation = removingNode ? observedNode(removingNode) : undefined
  const unsafeRemoval =
    removingNode?.validator === true && !validatorCanLeaveSafely(removalObservation)
  const participationEnabled = removalObservation?.participate_in_elections !== false
  const nodesView = view === "nodes" && config !== undefined
  const fallbackNodes = useMemo(() => config?.nodes.map(unobservedNode) ?? [], [config?.nodes])
  const nextNodeNumber = (config?.nodes.length ?? 0) + 1
  const suggestedNodeName = `${nodeIsValidator ? "validator" : "node"}-${nextNodeNumber}`

  const renderNodeActions = useCallback(
    (node: NodeView) => {
      const managed = managedNode(node)
      const genesis = node.name.toLowerCase() === "genesis"
      const actions: ReactNode[] = []

      if (managed) {
        const start = managed.stopped || !node.online
        actions.push(
          <InlineAction
            key="running"
            label={`${start ? "Start" : "Stop"} ${node.name}`}
            title={start ? "Start node" : "Stop node"}
            icon={start ? <Play /> : <Square />}
            disabled={changingNodeId !== undefined || environment?.status !== "running"}
            onClick={() => void setNodeRunning(managed, start)}
          />,
        )
        actions.push(
          node.participate_in_elections ? (
            <InlineAction
              key="leave-validation"
              label={`Leave validator set for ${node.name}`}
              title="Leave validator set"
              icon={<LogOut />}
              disabled={
                managed.stopped ||
                !node.online ||
                changingNodeId !== undefined ||
                leavingNodeId === managed.id
              }
              onClick={() => void leaveValidation(managed)}
            />
          ) : (
            <InlineAction
              key="enter-validation"
              label={`Enter elections for ${node.name}`}
              title="Enter elections"
              icon={<LogIn />}
              disabled={
                managed.stopped ||
                !node.online ||
                changingNodeId !== undefined ||
                enteringNodeId === managed.id
              }
              onClick={() => void enterValidation(managed)}
            />
          ),
        )
      }

      if (!node.online && !genesis && !managed) {
        actions.push(
          <InlineAction
            key="forget"
            label={`Forget ${node.name}`}
            title="Forget offline node"
            icon={<EyeOff />}
            disabled={forgettingObserverId === node.observer_id}
            onClick={() => void forgetNode(node)}
          />,
        )
      }

      if (genesis) {
        actions.push(
          <InlineAction
            key="running"
            label="Stop the environment to stop genesis"
            title="Stop the environment to stop the network owner"
            icon={<Square />}
            disabled
          />,
        )
        actions.push(
          <InlineAction
            key="remove"
            label="Network owner"
            title="The network owner cannot be removed"
            icon={<Trash2 />}
            variant="danger"
            disabled
          />,
        )
      } else if (managed) {
        actions.push(
          <InlineAction
            key="remove"
            label={`Remove ${node.name}`}
            title="Remove node"
            icon={<Trash2 />}
            variant="danger"
            disabled={changingNodeId !== undefined}
            onClick={() => setRemovingNode(managed)}
          />,
        )
      }

      return actions.length > 0 ? actions : null
    },
    [
      enterValidation,
      enteringNodeId,
      forgetNode,
      forgettingObserverId,
      leaveValidation,
      leavingNodeId,
      managedNode,
      changingNodeId,
      environment?.status,
      setNodeRunning,
    ],
  )

  return (
    <>
      <NetworkDashboard
        client={client}
        fallbackNodes={fallbackNodes}
        nodesFooter={
          nodesView ? (
            <Button
              size="sm"
              variant="secondary"
              leadingIcon={<Plus size={15} aria-hidden="true" />}
              onClick={() => {
                setNodeName(suggestedNodeName)
                setAddDialogOpen(true)
              }}
            >
              Add node
            </Button>
          ) : undefined
        }
        onAddressClick={(address, event) => {
          openExplorerPath(explorerRoutes.addressPath(address), event)
        }}
        onNetworkChange={setNetwork}
        renderNodeActions={nodesView ? renderNodeActions : undefined}
        view={view}
      />

      <Dialog
        busy={isAdding}
        className={styles.addNodePopup}
        maxWidth="28rem"
        open={addDialogOpen}
        onOpenChange={nextOpen => {
          if (!isAdding) setAddDialogOpen(nextOpen)
        }}
        title="Add node"
        description="Join another node to this local network"
      >
        <div className={styles.addNodeDialog}>
          <Input
            autoFocus
            aria-label="Node name"
            className={styles.addNodeNameInput}
            maxLength={64}
            placeholder={suggestedNodeName}
            value={nodeName}
            onChange={event => setNodeName(event.target.value)}
            onKeyDown={event => {
              if (event.key === "Enter" && nodeName.trim()) void addNode()
            }}
          />
          <Checkbox
            checked={nodeIsValidator}
            label="Validator"
            description="Participates in elections and starts validating after joining an elected set"
            onChange={event => {
              const validator = event.target.checked
              setNodeIsValidator(validator)
              setNodeName(current =>
                current === "" || /^(?:node|validator)-\d+$/.test(current)
                  ? `${validator ? "validator" : "node"}-${nextNodeNumber}`
                  : current,
              )
            }}
          />
          <DialogActions>
            <Button variant="outline" onClick={() => setAddDialogOpen(false)}>
              Cancel
            </Button>
            <Button
              variant="secondary"
              loading={isAdding}
              disabled={!nodeName.trim() || environment?.status !== "running"}
              leadingIcon={<Plus size={15} aria-hidden="true" />}
              onClick={() => void addNode()}
            >
              Add node
            </Button>
          </DialogActions>
        </div>
      </Dialog>

      <Dialog
        busy={isRemoving || leavingNodeId === removingNode?.id}
        maxWidth="32rem"
        open={removingNode !== undefined}
        onOpenChange={nextOpen => {
          if (!nextOpen) setRemovingNode(undefined)
        }}
        title={removingNode ? `Remove ${removingNode.name}` : "Remove node"}
        description={
          unsafeRemoval
            ? participationEnabled
              ? "This validator still belongs to an elected set"
              : "This validator is waiting for the elected set to change"
            : "The node container and its private chain state will be deleted"
        }
      >
        <div className={styles.confirmation}>
          <div className={styles.confirmationNode}>
            {removingNode?.validator ? <ShieldCheck size={18} /> : <Server size={18} />}
            <div>
              <strong>{removingNode?.name}</strong>
              <span>{removingNode?.validator ? "Validator" : "Full node"}</span>
            </div>
          </div>
          {unsafeRemoval ? (
            <div className={styles.forceWarning} role="alert">
              <TriangleAlert size={18} aria-hidden="true" />
              <span>
                {participationEnabled
                  ? "Leave the validator set, wait for the current round to finish, then try removing the node again"
                  : "Wait for the current round to finish, then try removing the node again"}
              </span>
            </div>
          ) : null}
          <DialogActions>
            <Button variant="outline" onClick={() => setRemovingNode(undefined)}>
              Cancel
            </Button>
            {unsafeRemoval && participationEnabled && removingNode ? (
              <Button
                variant="secondary"
                loading={leavingNodeId === removingNode.id}
                leadingIcon={<LogOut size={15} aria-hidden="true" />}
                onClick={() => void leaveValidation(removingNode)}
              >
                Leave validator set
              </Button>
            ) : null}
            {unsafeRemoval ? null : (
              <Button
                variant="danger"
                loading={isRemoving}
                leadingIcon={<Trash2 size={15} aria-hidden="true" />}
                onClick={() => void removeNode()}
              >
                Remove node
              </Button>
            )}
          </DialogActions>
        </div>
      </Dialog>
    </>
  )
}

/**
 * Keeps a managed node reachable when its collector report is absent after a restart
 * Unknown elected-set membership must still prevent unsafe removal
 */
function unobservedNode(node: FullTonNode): NodeView {
  return {
    observer_id: `managed:${node.id}`,
    name: node.name,
    generated_at: 0,
    expires_at: 0,
    online: false,
    running: false,
    sync_status: "offline",
    active_validator: false,
    validator_status: node.validator ? "unknown" : "not_configured",
    produced_masterchain_blocks: 0,
    produced_shard_blocks: 0,
    software: "",
    ton_release: "",
    observability_endpoint: "",
    instance_started_at: null,
    public_ip: "—",
    roles: ["full_node"],
    process_id: null,
    status: node.stopped ? "Stopped" : "Awaiting observation",
    last_error: null,
    head_seqno: null,
    head_observed_at: null,
    network_head_seqno: null,
    sync_initial_masterchain_block_time: null,
    sync_masterchain_block_time: null,
    sync_target_time: null,
    initial_sync_progress: null,
    sync_progressed_at: null,
    sync_lag_blocks: null,
    participate_in_elections: node.validator,
    current_validator: null,
    next_validator: null,
    location: {kind: "unavailable"},
    validator_public_key: null,
    validator_public_keys: [],
    validator_adnl: null,
    validator_stake_nano: null,
    validator_wallet_address: null,
    validator_wallet_version: null,
  }
}

function validatorCanLeaveSafely(node: NodeView | undefined) {
  return (
    node !== undefined &&
    !node.participate_in_elections &&
    !node.active_validator &&
    node.current_validator === false &&
    node.next_validator !== true
  )
}

function message(error: unknown, fallback: string) {
  return error instanceof Error && error.message ? error.message : fallback
}
