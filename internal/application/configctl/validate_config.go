package configctl

import (
	"context"
	"time"

	"internal/application/configctl/contracts"
	configdomain "internal/domain/configctl"
	"internal/shared/problem"
)

type ValidateConfigUseCase struct {
	repository Repository
	publisher  DomainEventPublisher
	now        func() time.Time
}

func NewValidateConfigUseCase(repository Repository, publisher DomainEventPublisher) *ValidateConfigUseCase {
	return &ValidateConfigUseCase{
		repository: repository,
		publisher:  publisher,
		now: func() time.Time {
			return time.Now().UTC()
		},
	}
}

func (uc *ValidateConfigUseCase) Execute(ctx context.Context, command contracts.ValidateConfigCommand) (contracts.ValidateConfigReply, *problem.Problem) {
	if uc == nil || uc.repository == nil {
		return contracts.ValidateConfigReply{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	command = command.Normalize()
	if prob := command.Validate(); prob != nil {
		return contracts.ValidateConfigReply{}, prob
	}

	set, prob := uc.repository.GetConfigSetByVersionID(ctx, command.VersionID)
	if prob != nil {
		return contracts.ValidateConfigReply{}, prob
	}
	before := snapshotConfigSet(set)

	diagnostics, prob := set.ValidateVersion(command.VersionID, uc.now())
	if prob != nil {
		return contracts.ValidateConfigReply{}, prob
	}
	version, _ := set.VersionByID(command.VersionID)
	if len(diagnostics) > 0 {
		return contracts.ValidateConfigReply{
			Config:      detailRecordFromDomain(set, version, nil),
			Valid:       false,
			Diagnostics: mapDiagnostics(diagnostics),
		}, nil
	}

	if prob := uc.repository.SaveConfigSet(ctx, set); prob != nil {
		return contracts.ValidateConfigReply{}, prob
	}
	if prob := publishEvents(ctx, uc.publisher, set.PullEvents()); prob != nil {
		_ = uc.repository.SaveConfigSet(ctx, before)
		return contracts.ValidateConfigReply{}, prob
	}

	return contracts.ValidateConfigReply{
		Config: detailRecordFromDomain(set, version, nil),
		Valid:  true,
	}, nil
}

func mapDiagnostics(diagnostics []configdomain.ValidationDiagnostic) []contracts.ValidationDiagnostic {
	if len(diagnostics) == 0 {
		return nil
	}
	reply := make([]contracts.ValidationDiagnostic, 0, len(diagnostics))
	for _, diagnostic := range diagnostics {
		reply = append(reply, contracts.ValidationDiagnostic{
			Field:   diagnostic.Field,
			Message: diagnostic.Message,
		})
	}
	return reply
}
