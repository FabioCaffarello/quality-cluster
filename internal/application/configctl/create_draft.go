package configctl

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"time"

	"internal/application/configctl/contracts"
	configdomain "internal/domain/configctl"
	"internal/shared/problem"
)

type CreateDraftUseCase struct {
	repository Repository
	publisher  DomainEventPublisher
	now        func() time.Time
	nextID     func() string
}

func NewCreateDraftUseCase(repository Repository, publisher DomainEventPublisher) *CreateDraftUseCase {
	return &CreateDraftUseCase{
		repository: repository,
		publisher:  publisher,
		now: func() time.Time {
			return time.Now().UTC()
		},
		nextID: newID,
	}
}

func (uc *CreateDraftUseCase) Execute(ctx context.Context, command contracts.CreateDraftCommand) (contracts.CreateDraftReply, *problem.Problem) {
	if uc == nil || uc.repository == nil {
		return contracts.CreateDraftReply{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	command = command.Normalize()
	if prob := command.Validate(); prob != nil {
		return contracts.CreateDraftReply{}, prob
	}

	now := uc.now()
	source := configdomain.ConfigSource{
		Format:  configdomain.SourceFormat(command.Format),
		Content: command.Content,
	}

	set, existing, prob := uc.loadOrCreateSet(ctx, command.Name, source, now)
	if prob != nil {
		return contracts.CreateDraftReply{}, prob
	}

	if prob := uc.repository.SaveConfigSet(ctx, set); prob != nil {
		return contracts.CreateDraftReply{}, prob
	}

	if prob := publishEvents(ctx, uc.publisher, set.PullEvents()); prob != nil {
		rollbackProb := uc.rollbackCreateDraft(ctx, set, existing)
		if rollbackProb != nil {
			return contracts.CreateDraftReply{}, rollbackProb
		}
		return contracts.CreateDraftReply{}, prob
	}

	version, _ := set.LatestVersion()
	return contracts.CreateDraftReply{
		Config: detailRecordFromDomain(set, version, nil),
	}, nil
}

func (uc *CreateDraftUseCase) loadOrCreateSet(ctx context.Context, key string, source configdomain.ConfigSource, now time.Time) (configdomain.ConfigSet, *configdomain.ConfigSet, *problem.Problem) {
	set, prob := uc.repository.GetConfigSetByKey(ctx, key)
	if prob != nil {
		if prob.Code != problem.NotFound {
			return configdomain.ConfigSet{}, nil, prob
		}
		newSet, createProb := configdomain.NewConfigSet(uc.nextID(), key, uc.nextID(), source, now)
		return newSet, nil, createProb
	}

	previous := snapshotConfigSet(set)
	if prob := set.CreateDraftVersion(uc.nextID(), source, now); prob != nil {
		return configdomain.ConfigSet{}, nil, prob
	}
	return set, &previous, nil
}

func (uc *CreateDraftUseCase) rollbackCreateDraft(ctx context.Context, set configdomain.ConfigSet, previous *configdomain.ConfigSet) *problem.Problem {
	if previous == nil {
		return uc.repository.DeleteConfigSet(ctx, set.ID)
	}
	return uc.repository.SaveConfigSet(ctx, *previous)
}

func newID() string {
	var raw [16]byte
	if _, err := rand.Read(raw[:]); err != nil {
		return time.Now().UTC().Format("20060102150405.000000000")
	}
	return hex.EncodeToString(raw[:])
}
