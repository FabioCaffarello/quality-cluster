package configctl

import (
	"context"

	"internal/application/configctl/contracts"
	configdomain "internal/domain/configctl"
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

	document, diagnostics, prob := configdomain.InspectDocument(configdomain.ConfigSource{
		Format:  configdomain.SourceFormat(command.Format),
		Content: command.Content,
	})
	if prob != nil {
		return contracts.ValidateDraftReply{}, prob
	}

	reply := contracts.ValidateDraftReply{
		Valid:              len(diagnostics) == 0,
		Diagnostics:        make([]contracts.ValidationDiagnostic, 0, len(diagnostics)),
		DefinitionChecksum: "",
	}
	for _, diagnostic := range diagnostics {
		reply.Diagnostics = append(reply.Diagnostics, contracts.ValidationDiagnostic{
			Field:   diagnostic.Field,
			Message: diagnostic.Message,
		})
	}
	if len(diagnostics) == 0 {
		reply.DefinitionChecksum = document.Checksum()
	}

	return reply, nil
}
