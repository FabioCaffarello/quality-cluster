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
	ListActiveRuntimeProjections(context.Context, contracts.ListActiveRuntimeProjectionsQuery) (contracts.ListActiveRuntimeProjectionsReply, *problem.Problem)
	ListActiveIngestionBindings(context.Context, contracts.ListActiveIngestionBindingsQuery) (contracts.ListActiveIngestionBindingsReply, *problem.Problem)
	ListConfigs(context.Context, contracts.ListConfigsQuery) (contracts.ListConfigsReply, *problem.Problem)
	ValidateDraft(context.Context, contracts.ValidateDraftCommand) (contracts.ValidateDraftReply, *problem.Problem)
	ValidateConfig(context.Context, contracts.ValidateConfigCommand) (contracts.ValidateConfigReply, *problem.Problem)
	CompileConfig(context.Context, contracts.CompileConfigCommand) (contracts.CompileConfigReply, *problem.Problem)
	ActivateConfig(context.Context, contracts.ActivateConfigCommand) (contracts.ActivateConfigReply, *problem.Problem)
}
