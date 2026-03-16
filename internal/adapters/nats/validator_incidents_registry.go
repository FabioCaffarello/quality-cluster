package nats

type ValidatorIncidentsRegistry struct {
	List ControlSpec
}

func DefaultValidatorIncidentsRegistry() ValidatorIncidentsRegistry {
	return ValidatorIncidentsRegistry{
		List: ControlSpec{
			Subject:     "validator.incidents.list",
			RequestType: "validator.incidents.query.list",
			ReplyType:   "validator.incidents.reply.list",
			QueueGroup:  "validator.incidents",
		},
	}
}
