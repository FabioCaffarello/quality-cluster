package configctl

import (
	"internal/application/configctl/contracts"
	"internal/shared/events"
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

type listActiveRuntimeProjectionsMessage struct {
	Query         contracts.ListActiveRuntimeProjectionsQuery
	CorrelationID string
}

type listActiveRuntimeProjectionsResult struct {
	Reply contracts.ListActiveRuntimeProjectionsReply
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

type listActiveIngestionBindingsMessage struct {
	Query         contracts.ListActiveIngestionBindingsQuery
	CorrelationID string
}

type listActiveIngestionBindingsResult struct {
	Reply contracts.ListActiveIngestionBindingsReply
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

type validateConfigMessage struct {
	Command       contracts.ValidateConfigCommand
	CorrelationID string
}

type validateConfigResult struct {
	Reply contracts.ValidateConfigReply
	Prob  *problem.Problem
}

type compileConfigMessage struct {
	Command       contracts.CompileConfigCommand
	CorrelationID string
}

type compileConfigResult struct {
	Reply contracts.CompileConfigReply
	Prob  *problem.Problem
}

type activateConfigMessage struct {
	Command       contracts.ActivateConfigCommand
	CorrelationID string
}

type activateConfigResult struct {
	Reply contracts.ActivateConfigReply
	Prob  *problem.Problem
}

type publishDomainEventMessage struct {
	Event events.Event
}

type publishDomainEventResult struct {
	Prob *problem.Problem
}
