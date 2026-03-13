package configctl

import (
	"internal/application/configctl/contracts"
	"internal/shared/problem"
)

type createDraftMessage struct {
	Command       contracts.CreateDraftCommand
	CorrelationID string
}

type createDraftResult struct {
	Reply contracts.CreateDraftReply
	Prob  *problem.Problem
}

type getConfigMessage struct {
	Query         contracts.GetConfigQuery
	CorrelationID string
}

type getConfigResult struct {
	Reply contracts.GetConfigReply
	Prob  *problem.Problem
}

type getActiveConfigMessage struct {
	Query         contracts.GetActiveConfigQuery
	CorrelationID string
}

type getActiveConfigResult struct {
	Reply contracts.GetActiveConfigReply
	Prob  *problem.Problem
}

type listConfigsMessage struct {
	Query         contracts.ListConfigsQuery
	CorrelationID string
}

type listConfigsResult struct {
	Reply contracts.ListConfigsReply
	Prob  *problem.Problem
}

type validateDraftMessage struct {
	Command       contracts.ValidateDraftCommand
	CorrelationID string
}

type validateDraftResult struct {
	Reply contracts.ValidateDraftReply
	Prob  *problem.Problem
}

type publishRuntimeEventMessage struct {
	Event contracts.RuntimeEvent
}

type publishRuntimeEventResult struct {
	Prob *problem.Problem
}
