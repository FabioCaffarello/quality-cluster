package configctlclient

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/application/ports"
	"internal/shared/problem"
)

type CreateDraftUseCase struct {
	gateway ports.ConfigctlGateway
}

func NewCreateDraftUseCase(gateway ports.ConfigctlGateway) *CreateDraftUseCase {
	return &CreateDraftUseCase{gateway: gateway}
}

func (uc *CreateDraftUseCase) Execute(ctx context.Context, command contracts.CreateDraftCommand) (contracts.CreateDraftReply, *problem.Problem) {
	if uc == nil || uc.gateway == nil {
		return contracts.CreateDraftReply{}, problem.New(problem.Unavailable, "config service is unavailable")
	}

	command = command.Normalize()
	if prob := command.Validate(); prob != nil {
		return contracts.CreateDraftReply{}, prob
	}

	return uc.gateway.CreateDraft(ctx, command)
}
