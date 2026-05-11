# 29. ZenUML Direct Fence

~~~zenuml
title Order Service
@Actor Client
@Boundary OrderController

@Starter(Client)
OrderController.post(payload) {
  return "created"
}
~~~
