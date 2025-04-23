UserInputSchema = {
  _required: %i[name email],
  name:     String,
  email:    String
}

UserResponseSchema = {
  _required: %i[id name email],
  id:        Integer,
  name:      String,
  email:     String
}

PostInputSchema = {
  _required: %i[title body],
  title:     String,
  body:      String
}

PostResponseSchema = {
  _required: %i[id title body],
  id:        Integer,
  title:     String,
  body:      String
}
