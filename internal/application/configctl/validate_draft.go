package configctl

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/domain/configuration"
	"internal/shared/problem"
)

type ValidateDraftUseCase struct{}

func NewValidateDraftUseCase() *ValidateDraftUseCase {
	return &ValidateDraftUseCase{}
}

func (uc *ValidateDraftUseCase) Execute(_ context.Context, command contracts.ValidateDraftCommand) (contracts.ValidateDraftReply, *problem.Problem) {
	command = command.Normalize()
	if prob := command.Validate(); prob != nil {
		return contracts.ValidateDraftReply{}, prob
	}

	diagnostics, prob := configuration.ValidateContent(configuration.Format(command.Format), command.Content)
	if prob != nil {
		return contracts.ValidateDraftReply{}, prob
	}

	reply := contracts.ValidateDraftReply{
		Valid:       len(diagnostics) == 0,
		Diagnostics: make([]contracts.ValidationDiagnostic, 0, len(diagnostics)),
	}
	for _, diagnostic := range diagnostics {
		reply.Diagnostics = append(reply.Diagnostics, contracts.ValidationDiagnostic{
			Field:   diagnostic.Field,
			Message: diagnostic.Message,
		})
	}

	return reply, nil
}
