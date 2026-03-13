package configctlclient

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/application/ports"
	"internal/shared/problem"
)

type ValidateDraftUseCase struct {
	gateway ports.ConfigctlGateway
}

func NewValidateDraftUseCase(gateway ports.ConfigctlGateway) *ValidateDraftUseCase {
	return &ValidateDraftUseCase{gateway: gateway}
}

func (uc *ValidateDraftUseCase) Execute(ctx context.Context, command contracts.ValidateDraftCommand) (contracts.ValidateDraftReply, *problem.Problem) {
	if uc == nil || uc.gateway == nil {
		return contracts.ValidateDraftReply{}, problem.New(problem.Unavailable, "config service is unavailable")
	}

	command = command.Normalize()
	if prob := command.Validate(); prob != nil {
		return contracts.ValidateDraftReply{}, prob
	}

	return uc.gateway.ValidateDraft(ctx, command)
}
