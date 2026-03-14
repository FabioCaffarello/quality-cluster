package configctl

import (
	"context"
	"time"

	"internal/application/configctl/contracts"
	"internal/shared/problem"
)

type ArchiveConfigUseCase struct {
	repository Repository
	publisher  DomainEventPublisher
	now        func() time.Time
}

func NewArchiveConfigUseCase(repository Repository, publisher DomainEventPublisher) *ArchiveConfigUseCase {
	return &ArchiveConfigUseCase{
		repository: repository,
		publisher:  publisher,
		now: func() time.Time {
			return time.Now().UTC()
		},
	}
}

func (uc *ArchiveConfigUseCase) Execute(ctx context.Context, command contracts.ArchiveConfigCommand) (contracts.ArchiveConfigReply, *problem.Problem) {
	if uc == nil || uc.repository == nil {
		return contracts.ArchiveConfigReply{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}
	command = command.Normalize()
	if prob := command.Validate(); prob != nil {
		return contracts.ArchiveConfigReply{}, prob
	}
	set, prob := uc.repository.GetConfigSetByVersionID(ctx, command.VersionID)
	if prob != nil {
		return contracts.ArchiveConfigReply{}, prob
	}
	before := snapshotConfigSet(set)
	if prob := set.ArchiveVersion(command.VersionID, uc.now()); prob != nil {
		return contracts.ArchiveConfigReply{}, prob
	}
	if prob := uc.repository.SaveConfigSet(ctx, set); prob != nil {
		return contracts.ArchiveConfigReply{}, prob
	}
	if prob := publishEvents(ctx, uc.publisher, set.PullEvents()); prob != nil {
		_ = uc.repository.SaveConfigSet(ctx, before)
		return contracts.ArchiveConfigReply{}, prob
	}
	version, _ := set.VersionByID(command.VersionID)
	activations, _ := uc.repository.ListActivationsByVersionID(ctx, command.VersionID)
	return contracts.ArchiveConfigReply{
		Config: detailRecordFromDomain(set, version, activations),
	}, nil
}
