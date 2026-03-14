package configctl

import (
	"context"
	"time"

	"internal/application/configctl/contracts"
	"internal/shared/problem"
)

type RejectConfigUseCase struct {
	repository Repository
	publisher  DomainEventPublisher
	now        func() time.Time
}

func NewRejectConfigUseCase(repository Repository, publisher DomainEventPublisher) *RejectConfigUseCase {
	return &RejectConfigUseCase{
		repository: repository,
		publisher:  publisher,
		now: func() time.Time {
			return time.Now().UTC()
		},
	}
}

func (uc *RejectConfigUseCase) Execute(ctx context.Context, command contracts.RejectConfigCommand) (contracts.RejectConfigReply, *problem.Problem) {
	if uc == nil || uc.repository == nil {
		return contracts.RejectConfigReply{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}
	command = command.Normalize()
	if prob := command.Validate(); prob != nil {
		return contracts.RejectConfigReply{}, prob
	}
	set, prob := uc.repository.GetConfigSetByVersionID(ctx, command.VersionID)
	if prob != nil {
		return contracts.RejectConfigReply{}, prob
	}
	before := snapshotConfigSet(set)
	if prob := set.RejectVersion(command.VersionID, command.Reason, uc.now()); prob != nil {
		return contracts.RejectConfigReply{}, prob
	}
	if prob := uc.repository.SaveConfigSet(ctx, set); prob != nil {
		return contracts.RejectConfigReply{}, prob
	}
	if prob := publishEvents(ctx, uc.publisher, set.PullEvents()); prob != nil {
		_ = uc.repository.SaveConfigSet(ctx, before)
		return contracts.RejectConfigReply{}, prob
	}
	version, _ := set.VersionByID(command.VersionID)
	activations, _ := uc.repository.ListActivationsByVersionID(ctx, command.VersionID)
	return contracts.RejectConfigReply{
		Config: detailRecordFromDomain(set, version, activations),
	}, nil
}
