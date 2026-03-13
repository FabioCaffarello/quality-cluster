package ports

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/shared/problem"
)

type ConfigctlGateway interface {
	CreateDraft(context.Context, contracts.CreateDraftCommand) (contracts.CreateDraftReply, *problem.Problem)
	GetConfig(context.Context, contracts.GetConfigQuery) (contracts.GetConfigReply, *problem.Problem)
	GetActiveConfig(context.Context, contracts.GetActiveConfigQuery) (contracts.GetActiveConfigReply, *problem.Problem)
	ListConfigs(context.Context, contracts.ListConfigsQuery) (contracts.ListConfigsReply, *problem.Problem)
	ValidateDraft(context.Context, contracts.ValidateDraftCommand) (contracts.ValidateDraftReply, *problem.Problem)
}
