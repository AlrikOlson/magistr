// Every mutation names both the resident session and its logical conversation.
export const addressKey = address => address ? JSON.stringify([address.project,address.workspace,address.session]) : "";
export const targetKey = target => target ? JSON.stringify([addressKey(target.address),target.conversation]) : "";
export const sameTarget = (left,right) => !!left && !!right && targetKey(left)===targetKey(right);
